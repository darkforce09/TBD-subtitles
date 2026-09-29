//! One process-shared cap for every headless Claude call.
//!
//! **Role:** admit a call only while fewer than the configured number are active.
//!
//! **Position:** private to the Claude CLI backend; the GUI sets its public limit.
//!
//! **Signals and state:** reads a cancellation flag and a persisted limit under app data;
//! advisory file locks hold the admission registry and each active call's slot.
//!
//! **Invariants:** a live limit change stops no active call; all historical slots count
//! toward a lowered cap; dropping a lock owner explicitly unlocks even when a fork inherits
//! its descriptor. An active call retains its permit until the backend finishes.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::LlmError;

const DEFAULT_LIMIT: usize = 32;
const POLL: Duration = Duration::from_millis(50);

/// Changes the cap shared by all Claude CLI backends and worker processes.
///
/// Set this from the saved application setting before starting jobs and whenever it
/// changes. Active calls finish normally; waiting calls observe the new cap. A zero
/// limit means one call, matching the in-process call gate.
pub fn set_shared_limit(limit: usize) -> Result<(), LlmError> {
    set_limit_at(&directory()?, limit)
}

pub(super) struct Permit {
    _slot: LockFile,
}

struct LockFile(File);

impl std::ops::Deref for LockFile {
    type Target = File;

    fn deref(&self) -> &File {
        &self.0
    }
}

impl Drop for LockFile {
    fn drop(&mut self) {
        // A fork can retain this open-file description before its close-on-exec takes effect.
        // Unlocking releases this owner's lock without waiting for that inherited handle.
        if let Err(error) = self.0.unlock() {
            tracing::warn!(%error, "Unable to release Claude call lock");
        }
    }
}

pub(super) fn acquire(cancel: Option<&AtomicBool>) -> Result<Permit, LlmError> {
    check_cancel(cancel)?;
    acquire_at(&directory()?, cancel)
}

fn directory() -> Result<PathBuf, LlmError> {
    crate::model_store::app_data_dir()
        .map(|path| path.join("claude-slots"))
        .map_err(|error| LlmError(format!("Claude call limit: {error}")))
}

fn set_limit_at(directory: &Path, limit: usize) -> Result<(), LlmError> {
    let registry = registry(directory)?;
    registry.lock().map_err(lock_error)?;
    let current = read_config(directory)?;
    let limit = limit.max(1);
    write_config(
        directory,
        Config {
            limit,
            slots: current.slots.max(limit),
        },
    )
}

fn acquire_at(directory: &Path, cancel: Option<&AtomicBool>) -> Result<Permit, LlmError> {
    loop {
        check_cancel(cancel)?;
        let registry = registry(directory)?;
        match registry.try_lock() {
            Ok(()) => {
                let config = read_config(directory)?;
                if let Some(permit) = available_slot(directory, config)? {
                    check_cancel(cancel)?;
                    return Ok(permit);
                }
            }
            Err(TryLockError::WouldBlock) => {}
            Err(TryLockError::Error(error)) => return Err(lock_error(error)),
        }
        drop(registry);
        std::thread::sleep(POLL);
    }
}

fn available_slot(directory: &Path, config: Config) -> Result<Option<Permit>, LlmError> {
    let mut first_free = None;
    let mut active = 0;
    // The registry lock serializes admissions. A running call can only release its
    // lock during this scan, so the count is conservative when a call finishes.
    for index in 0..config.slots {
        let slot = open_lock(&directory.join(format!("slot-{index}.lock")))?;
        match slot.try_lock() {
            Ok(()) if first_free.is_none() => first_free = Some(Permit { _slot: slot }),
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => active += 1,
            Err(TryLockError::Error(error)) => return Err(lock_error(error)),
        }
    }
    Ok(if active < config.limit {
        first_free
    } else {
        None
    })
}

#[derive(Clone, Copy)]
struct Config {
    limit: usize,
    slots: usize,
}

fn read_config(directory: &Path) -> Result<Config, LlmError> {
    let path = directory.join("limit.json");
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let limit = initial_limit()?;
            let config = Config {
                limit,
                slots: limit,
            };
            write_config(directory, config)?;
            return Ok(config);
        }
        Err(error) => return Err(file_error(&path, error)),
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes)
        .map_err(|error| LlmError(format!("Claude call limit {}: {error}", path.display())))?;
    let integer = |key| {
        value
            .get(key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|number| usize::try_from(number).ok())
            .filter(|number| *number > 0)
            .ok_or_else(|| {
                LlmError(format!(
                    "Claude call limit {}: invalid {key}",
                    path.display()
                ))
            })
    };
    let config = Config {
        limit: integer("limit")?,
        slots: integer("slots")?,
    };
    if config.slots < config.limit {
        return Err(LlmError(format!(
            "Claude call limit {}: slots is smaller than limit",
            path.display()
        )));
    }
    Ok(config)
}

fn initial_limit() -> Result<usize, LlmError> {
    match std::env::var("TBD_CLAUDE_CALL_LIMIT") {
        Ok(value) => value
            .parse::<usize>()
            .map(|limit| limit.max(1))
            .map_err(|_| LlmError("TBD_CLAUDE_CALL_LIMIT must be a nonnegative integer".into())),
        Err(std::env::VarError::NotPresent) => Ok(DEFAULT_LIMIT),
        Err(error) => Err(LlmError(format!("TBD_CLAUDE_CALL_LIMIT: {error}"))),
    }
}

fn write_config(directory: &Path, config: Config) -> Result<(), LlmError> {
    let path = directory.join("limit.json");
    let part = directory.join("limit.json.part");
    let bytes = serde_json::to_vec(&serde_json::json!({
        "limit": config.limit,
        "slots": config.slots,
    }))
    .map_err(|error| LlmError(format!("Claude call limit: {error}")))?;
    std::fs::write(&part, bytes).map_err(|error| file_error(&part, error))?;
    std::fs::rename(&part, &path).map_err(|error| file_error(&path, error))
}

fn registry(directory: &Path) -> Result<LockFile, LlmError> {
    std::fs::create_dir_all(directory).map_err(|error| file_error(directory, error))?;
    open_lock(&directory.join("registry.lock"))
}

fn open_lock(path: &Path) -> Result<LockFile, LlmError> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map(LockFile)
        .map_err(|error| file_error(path, error))
}

fn check_cancel(cancel: Option<&AtomicBool>) -> Result<(), LlmError> {
    if cancel.is_some_and(|flag| flag.load(Ordering::Relaxed)) {
        Err(LlmError("cancelled".into()))
    } else {
        Ok(())
    }
}

fn lock_error(error: std::io::Error) -> LlmError {
    LlmError(format!("Claude call limit lock: {error}"))
}

fn file_error(path: &Path, error: std::io::Error) -> LlmError {
    LlmError(format!("Claude call limit {}: {error}", path.display()))
}

#[cfg(test)]
#[path = "tests/shared_slots.rs"]
mod tests;

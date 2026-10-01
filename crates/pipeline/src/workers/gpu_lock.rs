//! The machine-wide GPU lock: one GPU worker at a time across every process of the app, so a
//! command-line run and the window never load models onto the card together, and two GPU steps
//! of one job (the main walk's and the visual lane's) never load theirs together either. While a
//! main-walk step waits for it, no visual-lane step takes it.
//!
//! **Role:** hold an exclusive `flock` on `gpu.lock` in the app data folder while a GPU worker
//! runs; wait for it, polling, while another holder has it; let an `Audio` waiter go before every
//! `Visual` one; name to the waiter the step of this process that holds it, when one does.
//!
//! **Position:** called by `workers::run_worker` for steps that lock the GPU up front, and by
//! `workers::lazy_gpu` in the translation worker when its local model loads; the priority comes
//! from `graph::gpu_priority`.
//!
//! **Signals and state:** opens and locks the lock file, each `acquire` on a descriptor of its
//! own, so two holders of one process conflict as two processes do; the kernel releases the lock
//! when the holder's file closes or its process dies, so a killed holder never leaves it stuck.
//! An `Audio` acquirer that has to wait holds a shared `flock` on the waiting file beside the lock
//! (`gpu.lock.audio`) until its wait ends; a `Visual` acquirer that cannot take that file
//! exclusively knows an `Audio` waiter exists, in this process or another, and keeps waiting. A
//! process-wide table names the step holding each lock file this process holds, changed only
//! under the same mutex as the `flock` calls.
//!
//! **Invariants:** a cancelled wait returns a cancelled error and never takes the lock; a `Visual`
//! acquirer never takes the lock while an `Audio` acquirer waits for it; a waiting `Audio`
//! acquirer's mark goes when its wait ends, taken, cancelled or failed; a lock this process holds
//! is in the table from the moment it is taken until it is released, so a waiter of this process
//! that finds it held and no holder named knows another process holds it.

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use job_model::StepName;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::GpuPriority;

/// How often a waiting run tries the lock again.
const POLL: Duration = Duration::from_millis(250);

/// The step of this process that holds a GPU lock, and the work directory of its job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Holder {
    pub step: StepName,
    pub job: PathBuf,
}

/// The GPU locks this process holds, by lock file, with the step holding each.
static HELD: Mutex<Vec<(PathBuf, Holder)>> = Mutex::new(Vec::new());

/// The lock, held until dropped.
#[derive(Debug)]
pub struct GpuLock {
    file: File,
    path: PathBuf,
}

impl Drop for GpuLock {
    fn drop(&mut self) {
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: `flock` on the descriptor this lock owns; it only reads the descriptor.
        unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
        if let Some(index) = held.iter().position(|(path, _)| *path == self.path) {
            held.remove(index);
        }
    }
}

/// The file beside the lock at `path` whose shared `flock`s mark the `Audio` acquirers waiting
/// for it.
pub fn audio_waiting_path(path: &Path) -> PathBuf {
    let mut name = OsString::from(path.as_os_str());
    name.push(".audio");
    PathBuf::from(name)
}

/// Take the lock at `path` for `holder` with `priority`, calling `waiting` once, with the step of
/// this process holding it or `None` when another process does, if it is held, and giving up when
/// `cancel` is set. A `Visual` acquirer also waits while an `Audio` one does.
pub fn acquire(
    path: &Path,
    holder: Holder,
    priority: GpuPriority,
    cancel: &CancelToken,
    waiting: &dyn Fn(Option<&Holder>),
) -> Result<GpuLock> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).context(format!("cannot create {}", folder.display()))?;
    }
    let file = open(path)?;
    let audio_waiting = audio_waiting_path(path);
    // An `Audio` waiter's mark, held until this function returns.
    let mut marked: Option<File> = None;
    let mut told = false;
    loop {
        if cancel.is_cancelled() {
            return Err(PipelineError::cancelled("waiting for the GPU"));
        }
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        let yielding = priority == GpuPriority::Visual && audio_waits(&audio_waiting)?;
        // SAFETY: `flock` on a descriptor this function owns; it only reads the descriptor.
        let taken = !yielding
            && unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
        if taken {
            held.push((path.to_path_buf(), holder));
            return Ok(GpuLock {
                file,
                path: path.to_path_buf(),
            });
        }
        if !yielding {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
                return Err(PipelineError::new(
                    format!("lock {}", path.display()),
                    error,
                ));
            }
        }
        let current = held
            .iter()
            .find(|(locked, _)| locked == path)
            .map(|(_, holder)| holder.clone());
        drop(held);
        if priority == GpuPriority::Audio && marked.is_none() {
            marked = Some(mark_waiting(&audio_waiting)?);
        }
        if !told {
            waiting(current.as_ref());
            told = true;
        }
        std::thread::sleep(POLL);
    }
}

/// Open (creating) the file at `path` for `flock`.
fn open(path: &Path) -> Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .context(format!("cannot open {}", path.display()))
}

/// Mark an `Audio` acquirer as waiting: a shared `flock` on the waiting file, held while the
/// returned file is open.
fn mark_waiting(audio_waiting: &Path) -> Result<File> {
    let file = open(audio_waiting)?;
    // SAFETY: `flock` on a descriptor this function owns; it only reads the descriptor. A
    // `Visual` acquirer holds the file exclusively only between two calls, so this returns at
    // once.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_SH) } != 0 {
        return Err(PipelineError::new(
            format!("lock {}", audio_waiting.display()),
            std::io::Error::last_os_error(),
        ));
    }
    Ok(file)
}

/// Whether an `Audio` acquirer waits: the waiting file cannot be taken exclusively. A file taken
/// to find out is released at once.
fn audio_waits(audio_waiting: &Path) -> Result<bool> {
    let file = open(audio_waiting)?;
    // SAFETY: `flock` on a descriptor this function owns; it only reads the descriptor.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        // SAFETY: as above.
        unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_UN) };
        return Ok(false);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
        Ok(true)
    } else {
        Err(PipelineError::new(
            format!("lock {}", audio_waiting.display()),
            error,
        ))
    }
}

/// The line a step waiting for the GPU shows: the step holding it when it is one of this
/// process's, of this job (`job`) or another, else another run of the app.
pub fn waiting_message(holder: Option<&Holder>, job: &Path) -> String {
    match holder {
        Some(holder) if holder.job == job => {
            format!(
                "waiting for the GPU: {} of this job is using it",
                holder.step
            )
        }
        Some(holder) => format!(
            "waiting for the GPU: {} of another job is using it",
            holder.step
        ),
        None => "waiting for the GPU: another run of the app is using it".to_string(),
    }
}

#[cfg(test)]
#[path = "tests/gpu_lock.rs"]
mod tests;

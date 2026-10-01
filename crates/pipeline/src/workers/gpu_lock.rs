//! The machine-wide GPU lock: one GPU worker at a time across every process of the app, so a
//! command-line run and the window never load models onto the card together, and two GPU steps
//! of one job (the main walk's and the visual lane's) never load theirs together either.
//!
//! **Role:** hold an exclusive `flock` on `gpu.lock` in the app data folder while a GPU worker
//! runs; wait for it, polling, while another holder has it; name to the waiter the step of this
//! process that holds it, when one does.
//!
//! **Position:** called by `workers::run_worker` for steps that use the GPU.
//!
//! **Signals and state:** opens and locks the lock file, each `acquire` on a descriptor of its
//! own, so two holders of one process conflict as two processes do; the kernel releases the lock when the holder's file closes or its process dies, so a killed
//! holder never leaves it stuck; a process-wide table names the step holding each lock file this
//! process holds, changed only under the same mutex as the `flock` calls.
//!
//! **Invariants:** a cancelled wait returns a cancelled error and never takes the lock; a lock
//! this process holds is in the table from the moment it is taken until it is released, so a
//! waiter of this process that finds it held and no holder named knows another process holds it.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use job_model::StepName;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};

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

/// Take the lock at `path` for `holder`, calling `waiting` once, with the step of this process
/// holding it or `None` when another process does, if it is held, and giving up when `cancel` is
/// set.
pub fn acquire(
    path: &Path,
    holder: Holder,
    cancel: &CancelToken,
    waiting: &dyn Fn(Option<&Holder>),
) -> Result<GpuLock> {
    if let Some(folder) = path.parent() {
        std::fs::create_dir_all(folder).context(format!("cannot create {}", folder.display()))?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .context(format!("cannot open {}", path.display()))?;
    let mut told = false;
    loop {
        if cancel.is_cancelled() {
            return Err(PipelineError::cancelled("waiting for the GPU"));
        }
        let mut held = HELD.lock().unwrap_or_else(PoisonError::into_inner);
        // SAFETY: `flock` on a descriptor this function owns; it only reads the descriptor.
        let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
        if taken {
            held.push((path.to_path_buf(), holder));
            return Ok(GpuLock {
                file,
                path: path.to_path_buf(),
            });
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
            return Err(PipelineError::new(
                format!("lock {}", path.display()),
                error,
            ));
        }
        let current = held
            .iter()
            .find(|(locked, _)| locked == path)
            .map(|(_, holder)| holder.clone());
        drop(held);
        if !told {
            waiting(current.as_ref());
            told = true;
        }
        std::thread::sleep(POLL);
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

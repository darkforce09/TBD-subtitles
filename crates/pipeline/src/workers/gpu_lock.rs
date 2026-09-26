//! The machine-wide GPU lock: one GPU worker at a time across every process of the app, so a
//! command-line run and the window never load models onto the card together.
//!
//! **Role:** hold an exclusive `flock` on `gpu.lock` in the app data folder while a GPU worker
//! runs; wait for it, polling, while another process holds it.
//!
//! **Position:** called by `workers::run_worker` for steps that use the GPU.
//!
//! **Signals and state:** opens and locks the lock file; the kernel releases the lock when the
//! holder's file closes or its process dies, so a killed holder never leaves it stuck.
//!
//! **Invariants:** a cancelled wait returns a cancelled error and never takes the lock.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::Duration;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};

/// How often a waiting run tries the lock again.
const POLL: Duration = Duration::from_millis(250);

/// The lock, held until dropped.
#[derive(Debug)]
pub struct GpuLock {
    _file: File,
}

/// Take the lock at `path`, calling `waiting` once if another process holds it, and giving up
/// when `cancel` is set.
pub fn acquire(path: &Path, cancel: &CancelToken, waiting: &dyn Fn()) -> Result<GpuLock> {
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
        // SAFETY: `flock` on a descriptor this function owns; it only reads the descriptor.
        let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0;
        if taken {
            return Ok(GpuLock { _file: file });
        }
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() != Some(libc::EWOULDBLOCK) {
            return Err(PipelineError::new(
                format!("lock {}", path.display()),
                error,
            ));
        }
        if !told {
            waiting();
            told = true;
        }
        std::thread::sleep(POLL);
    }
}

#[cfg(test)]
#[path = "tests/gpu_lock.rs"]
mod tests;

//! The switch that stops a running job: the window sets it, the runner checks it between steps,
//! and every worker's watchdog kills its process group once it is set.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A shared stop flag; clones share it.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> CancelToken {
        CancelToken::default()
    }

    /// Ask the job to stop: the running worker is killed and no further step starts.
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// The flag itself, for a child process's watchdog.
    pub fn flag(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }
}

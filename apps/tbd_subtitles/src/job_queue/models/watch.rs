//! What the watch folders' scans remember between one scan and the next.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::SystemTime;

/// A video's size and modification time at one scan; a video whose sample matches the one from
/// the scan before has stopped changing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sample {
    pub(crate) size: u64,
    pub(crate) modified: SystemTime,
}

/// The state of a run of scans over the watch folders.
#[derive(Debug, Default)]
pub(crate) struct WatchScan {
    /// Each video seen by the last scan, with its sample then.
    pub(crate) samples: HashMap<PathBuf, Sample>,
    /// Every video reported as complete, never reported again.
    pub(crate) reported: HashSet<PathBuf>,
    /// The watch folders missing at the last scan, each warned about once until it reappears.
    pub(crate) missing: HashSet<PathBuf>,
}

//! One scan of the watch folders: the videos under them that have stopped changing since the
//! scan before, each reported once.
//!
//! **Role:** sample each video under the watch folders (`video_files::videos_under`) by its size
//! and modification time, and report those whose sample matches the previous scan's.
//!
//! **Position:** called by `folder_watcher`'s thread on each tick; `step` is its pure core.
//!
//! **Signals and state:** reads the folders and file metadata; keeps the last samples, the
//! videos reported and the folders found missing in the `WatchScan` it is given; warns once when
//! a watch folder is missing.
//!
//! **Invariants:** a video is never reported on its first sighting, only once two scans in a row
//! see the same sample; each video is reported at most once per `WatchScan`, even after it
//! vanishes and comes back; samples of videos no longer seen are dropped; the report is sorted.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::job_queue::models::watch::{Sample, WatchScan};
use crate::job_queue::services::video_files;

impl WatchScan {
    /// Scan `folders` and return the videos under them that have stopped changing since the last
    /// scan and were not reported before, sorted. A missing folder is skipped, and warned about
    /// once until it comes back.
    pub(crate) fn scan(&mut self, folders: &[PathBuf]) -> Vec<PathBuf> {
        let mut seen = BTreeMap::new();
        for folder in folders {
            if !folder.is_dir() {
                if self.missing.insert(folder.clone()) {
                    tracing::warn!(folder = %folder.display(), "a watch folder is missing");
                }
                continue;
            }
            self.missing.remove(folder);
            for video in video_files::videos_under(folder) {
                let Ok(metadata) = std::fs::metadata(&video) else {
                    continue;
                };
                let Ok(modified) = metadata.modified() else {
                    continue;
                };
                let sample = Sample {
                    size: metadata.len(),
                    modified,
                };
                seen.insert(video, sample);
            }
        }
        step(self, seen.into_iter().collect())
    }
}

/// Fold one scan's `seen` videos into `scan`: report each whose sample equals the previous
/// scan's and that was not reported before, keep only the samples seen now, and return the
/// report sorted.
pub(crate) fn step(scan: &mut WatchScan, seen: Vec<(PathBuf, Sample)>) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut samples = std::collections::HashMap::with_capacity(seen.len());
    for (path, sample) in seen {
        if scan.samples.get(&path) == Some(&sample) && !scan.reported.contains(&path) {
            found.push(path.clone());
        }
        samples.insert(path, sample);
    }
    scan.samples = samples;
    found.sort();
    found.dedup();
    scan.reported.extend(found.iter().cloned());
    found
}

#[cfg(test)]
#[path = "tests/watch_scan.rs"]
mod tests;

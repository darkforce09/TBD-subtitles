//! The work folder's size: every job's work directory, measured on a thread of its own.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use crate::core::background::Wake;

/// The bytes of every file under `folder`, symbolic links not followed; 0 for a missing folder.
pub(crate) fn size(folder: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| match entry.file_type() {
            Ok(kind) if kind.is_dir() => size(&entry.path()),
            Ok(kind) if kind.is_file() => entry.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}

/// Measure `folder` on a thread; the size arrives on the returned channel, then `wake` runs.
pub(crate) fn measure(folder: PathBuf, wake: Wake) -> Receiver<u64> {
    let (send, answer) = channel();
    std::thread::spawn(move || {
        let _ = send.send(size(&folder));
        wake();
    });
    answer
}

#[cfg(test)]
#[path = "tests/work_folder.rs"]
mod tests;

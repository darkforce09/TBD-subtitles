//! The thread that watches the watch folders: it scans them on a fixed interval and hands the
//! window each video that has finished arriving.
//!
//! **Role:** run a `WatchScan` over the watch folders every `interval`, and at once when the
//! folders change, sending each non-empty list of complete videos and waking the window.
//!
//! **Position:** started by the application with the watch folders from the settings; the
//! application drains `found` and hands `set_folders` the folders whenever the settings change.
//!
//! **Signals and state:** one named thread, "folder-watcher", holding the scan's state; a
//! channel of folder lists in, a channel of found videos out; reads the folders' listings.
//!
//! **Invariants:** an empty folder list scans nothing; a video is sent at most once per watcher;
//! the thread ends as soon as the watcher is dropped.

use std::path::PathBuf;
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, channel};
use std::time::Duration;

use crate::core::background::Wake;
use crate::job_queue::models::watch::WatchScan;

/// How often the watch folders are scanned.
pub(crate) const WATCH_INTERVAL: Duration = Duration::from_secs(15);

/// The handle of the watching thread.
pub(crate) struct FolderWatcher {
    /// Each scan's complete videos, sorted, when there are any.
    pub(crate) found: Receiver<Vec<PathBuf>>,
    folders: Vec<PathBuf>,
    commands: Sender<Vec<PathBuf>>,
}

/// Start watching `folders`, scanning them at once and then every `interval`; `wake` is called
/// after each list of videos is sent.
pub(crate) fn start(folders: Vec<PathBuf>, interval: Duration, wake: Wake) -> FolderWatcher {
    let (commands, inbox) = channel::<Vec<PathBuf>>();
    let (send, found) = channel();
    let watched = folders.clone();
    std::thread::Builder::new()
        .name("folder-watcher".to_string())
        .spawn(move || watch(watched, interval, &inbox, &send, &wake))
        .map(|_| ())
        .unwrap_or_else(|error| tracing::error!(%error, "the folder watcher could not start"));
    FolderWatcher {
        found,
        folders,
        commands,
    }
}

impl FolderWatcher {
    /// The folders being watched.
    pub(crate) fn folders(&self) -> &[PathBuf] {
        &self.folders
    }

    /// Watch `folders` instead, scanning them at once; nothing happens when they are the ones
    /// watched already.
    pub(crate) fn set_folders(&mut self, folders: &[PathBuf]) {
        if self.folders == folders {
            return;
        }
        self.folders = folders.to_vec();
        let _ = self.commands.send(self.folders.clone());
    }
}

/// The thread's loop: scan, then wait for new folders or the interval, until the watcher is
/// dropped or the window stops listening.
fn watch(
    mut folders: Vec<PathBuf>,
    interval: Duration,
    inbox: &Receiver<Vec<PathBuf>>,
    send: &Sender<Vec<PathBuf>>,
    wake: &Wake,
) {
    let mut scan = WatchScan::default();
    loop {
        if !folders.is_empty() {
            let videos = scan.scan(&folders);
            if !videos.is_empty() {
                if send.send(videos).is_err() {
                    return;
                }
                wake();
            }
        }
        match inbox.recv_timeout(interval) {
            Ok(list) => folders = list,
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[cfg(test)]
#[path = "tests/folder_watcher.rs"]
mod tests;

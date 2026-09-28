//! The models and runtime libraries a job needs: which are on disk, and downloading the missing
//! ones on a thread of their own, with progress and a stop switch.
//!
//! **Role:** list every model folder the settings need and every runtime archive the GPU workers
//! load, each with its size and whether it is present; download the missing ones in that order,
//! reporting each item by its id; fold those reports into the page.
//!
//! **Position:** called by the application for the Settings window and the models banner, and
//! before a job starts; uses `pipeline::models` for the list and `inference::model_store` for the
//! pinned downloads.
//!
//! **Signals and state:** reads the models and runtime folders; the download thread writes into
//! them and sends `DownloadEvent`s, then wakes the window.
//!
//! **Invariants:** only pinned files are downloaded, each checked against its SHA-256 before it is
//! used; a stopped download keeps its part file, so the next one resumes it; an event names its
//! item by id, so a list planned again while a download runs is marked right.

use std::ops::ControlFlow;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use inference::cuda_runtime::CudaRuntime;
use inference::model_store::{self, CUDA_ARCHIVES, ONNX_RUNTIME_ARCHIVE, PinnedArchive, manifest};
use job_model::job::JobSettings;

use crate::core::background::Wake;
use crate::settings::models::machine::{DownloadItem, ItemKind};
use crate::settings::models::page::{DownloadProgress, SettingsPage};

/// Where the items go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Folders {
    pub(crate) models: PathBuf,
    pub(crate) runtime: PathBuf,
    /// The running binary's folder, whose `cuda/` is looked in first for the runtime.
    pub(crate) exe_dir: Option<PathBuf>,
}

/// What the download thread reports, each item by its id.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DownloadEvent {
    /// `held` of `total` bytes of item `id` are on disk.
    Advanced { id: String, held: u64, total: u64 },
    /// Item `id`, of `bytes`, is on disk.
    Finished { id: String, bytes: u64 },
    /// The download ended.
    Ended(DownloadEnd),
}

/// How a download ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DownloadEnd {
    /// Every item it was given is on disk.
    Done,
    /// The owner stopped it; the part file stays for the next attempt.
    Stopped,
    /// It failed, for this reason.
    Failed(String),
}

/// A running download: its events and its stop switch.
pub(crate) struct Downloading {
    pub(crate) events: Receiver<DownloadEvent>,
    stop: Arc<AtomicBool>,
}

impl Downloading {
    /// Stop after the current block; the part file stays for the next attempt.
    pub(crate) fn stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// The runtime archives the app's workers load: the CUDA 13 libraries and ONNX Runtime; the
/// build-only toolkit is left out.
pub(crate) fn runtime_archives() -> impl Iterator<Item = &'static PinnedArchive> {
    CUDA_ARCHIVES
        .iter()
        .chain(std::iter::once(&ONNX_RUNTIME_ARCHIVE))
}

/// Every item a job with `settings` needs, the models first, each marked present or not. When a
/// complete runtime is found (packaged beside the binary or in the runtime folder), every runtime
/// archive counts as present.
pub(crate) fn plan(folders: &Folders, settings: &JobSettings) -> Vec<DownloadItem> {
    let runtime_found = CudaRuntime::locate(folders.exe_dir.as_deref(), &folders.runtime).is_ok();
    let models = pipeline::models::required(settings)
        .into_iter()
        .map(|id| DownloadItem {
            kind: ItemKind::Model,
            id: id.to_string(),
            bytes: manifest::files_of(id).map(|f| f.size).sum(),
            present: model_store::is_complete(&folders.models, id),
        });
    let runtime = runtime_archives().map(|archive| DownloadItem {
        kind: ItemKind::Runtime,
        id: archive.id.to_string(),
        bytes: archive.size,
        present: runtime_found || model_store::is_archive_installed(archive, &folders.runtime),
    });
    models.chain(runtime).collect()
}

/// The progress of a download of `items` as it starts: at the first missing item, nothing held;
/// `None` when nothing is missing.
pub(crate) fn begun(items: &[DownloadItem]) -> Option<DownloadProgress> {
    let mut missing = items.iter().filter(|item| !item.present);
    let first = missing.next()?;
    Some(DownloadProgress {
        id: first.id.clone(),
        held: 0,
        total: first.bytes,
        finished: 0,
        size: first.bytes + missing.map(|item| item.bytes).sum::<u64>(),
    })
}

/// Fold one event of the running download into `page`, finding its item by id wherever the item
/// sits in the list now; how the download ended, once it has.
pub(crate) fn fold(page: &mut SettingsPage, event: DownloadEvent) -> Option<DownloadEnd> {
    match event {
        DownloadEvent::Advanced { id, held, total } => {
            if let Some(progress) = &mut page.download {
                progress.id = id;
                progress.held = held;
                progress.total = total;
            }
            None
        }
        DownloadEvent::Finished { id, bytes } => {
            for item in page.items.iter_mut().filter(|item| item.id == id) {
                item.present = true;
            }
            if let Some(progress) = &mut page.download {
                progress.finished += bytes;
                if progress.id == id {
                    progress.held = 0;
                }
            }
            None
        }
        DownloadEvent::Ended(end) => Some(end),
    }
}

/// Download every missing item of `items` on a new thread; `wake` is called after each event.
pub(crate) fn start(items: Vec<DownloadItem>, folders: Folders, wake: Wake) -> Downloading {
    let (send, events) = channel();
    let stop = Arc::new(AtomicBool::new(false));
    let flag = stop.clone();
    std::thread::spawn(move || {
        let end = download_all(&items, &folders, &flag, &send, &wake);
        let _ = send.send(DownloadEvent::Ended(end));
        wake();
    });
    Downloading { events, stop }
}

fn download_all(
    items: &[DownloadItem],
    folders: &Folders,
    stop: &AtomicBool,
    send: &Sender<DownloadEvent>,
    wake: &Wake,
) -> DownloadEnd {
    for item in items.iter().filter(|i| !i.present) {
        // A model's files download in manifest order; the files before one count as held.
        let offset = |file: &str| -> u64 {
            manifest::files_of(&item.id)
                .take_while(|f| f.file != file)
                .map(|f| f.size)
                .sum()
        };
        let report = |file: &str, held: u64| {
            let _ = send.send(DownloadEvent::Advanced {
                id: item.id.clone(),
                held: (offset(file) + held).min(item.bytes),
                total: item.bytes,
            });
            wake();
            if stop.load(Ordering::SeqCst) {
                ControlFlow::Break(())
            } else {
                ControlFlow::Continue(())
            }
        };
        let result = match item.kind {
            ItemKind::Model => {
                model_store::fetch_model(&folders.models, &item.id, &mut |f, held, _| {
                    report(f.file, held)
                })
                .map(|_| ())
            }
            ItemKind::Runtime => {
                let Some(archive) = runtime_archives().find(|a| a.id == item.id) else {
                    return DownloadEnd::Failed(format!("no runtime archive named {}", item.id));
                };
                model_store::install_archive(archive, &folders.runtime, &mut |held, _| {
                    report(archive.id, held)
                })
            }
        };
        match result {
            Ok(()) => {
                let _ = send.send(DownloadEvent::Finished {
                    id: item.id.clone(),
                    bytes: item.bytes,
                });
                wake();
            }
            Err(model_store::StoreError::Cancelled) => return DownloadEnd::Stopped,
            Err(error) => return DownloadEnd::Failed(format!("{}: {error}", item.id)),
        }
    }
    DownloadEnd::Done
}

#[cfg(test)]
#[path = "tests/model_downloads.rs"]
mod tests;

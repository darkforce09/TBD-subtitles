//! What the Settings window's Models tab and the models banner say about the models: one row per
//! model and runtime library (the CUDA archives as one row), what is missing, and which banner
//! shows under the toolbar.
//!
//! **Role:** turn the page's items and download into rows, a count of what is missing (models
//! and runtime libraries apart) with its words, and the banner.
//!
//! **Position:** read by `settings::ui::{models_tab, models_banner}` and by the application's
//! frame, which shows the banner.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** the counts are of the rows the Models tab lists, so the banner and the tab
//! agree; a download that runs shows its banner even while items are missing.

use std::time::{Duration, Instant};

use inference::model_store::CUDA_ARCHIVES;

use crate::core::format;
use crate::settings::models::machine::{DownloadItem, ItemKind};
use crate::settings::models::page::{DownloadProgress, SettingsPage};

/// How long the banner says every model is on disk after a download.
pub(crate) const ALL_ON_DISK_SHOWN: Duration = Duration::from_secs(4);

/// How a row of the Models tab stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum RowState {
    OnDisk,
    Missing,
    /// Downloading, this share (0 to 1) on disk.
    Downloading(f32),
}

/// One row of the Models tab: a model folder, or a runtime library.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ModelRow {
    pub(crate) name: String,
    pub(crate) kind: ItemKind,
    pub(crate) bytes: u64,
    pub(crate) state: RowState,
}

/// What is missing: model folders and runtime libraries, counted as rows, and their bytes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Missing {
    pub(crate) models: usize,
    pub(crate) runtime: usize,
    pub(crate) bytes: u64,
}

impl Missing {
    pub(crate) fn any(&self) -> bool {
        self.models + self.runtime > 0
    }

    /// "5 models are missing (11.4 GiB)", or "5 models and 2 runtime libraries are missing
    /// (13.8 GiB)" when both are.
    pub(crate) fn headline(&self) -> String {
        let models = match self.models {
            1 => "1 model".to_string(),
            n => format!("{n} models"),
        };
        let runtime = match self.runtime {
            1 => "1 runtime library".to_string(),
            n => format!("{n} runtime libraries"),
        };
        let (what, one) = match (self.models, self.runtime) {
            (_, 0) => (models, self.models == 1),
            (0, _) => (runtime, self.runtime == 1),
            _ => (format!("{models} and {runtime}"), false),
        };
        let verb = if one { "is" } else { "are" };
        format!("{what} {verb} missing ({})", format::size(self.bytes))
    }
}

/// The banner under the toolbar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Banner {
    /// Something is missing and nothing downloads.
    Missing(Missing),
    /// A download runs: the item now and the whole download.
    Downloading(DownloadProgress),
    /// A download brought every item onto disk; the banner goes at `until`.
    AllOnDisk { until: Instant },
}

/// The rows of the Models tab: each model, then the CUDA archives as one row, then the other
/// runtime libraries.
pub(crate) fn rows(items: &[DownloadItem], download: Option<&DownloadProgress>) -> Vec<ModelRow> {
    let is_cuda = |item: &&DownloadItem| CUDA_ARCHIVES.iter().any(|a| a.id == item.id);
    let mut rows: Vec<ModelRow> = items
        .iter()
        .filter(|item| item.kind == ItemKind::Model)
        .map(|item| row(item.id.clone(), ItemKind::Model, &[item], download))
        .collect();
    let cuda: Vec<&DownloadItem> = items.iter().filter(is_cuda).collect();
    if !cuda.is_empty() {
        let name = format!("CUDA and cuDNN ({} archives)", cuda.len());
        rows.push(row(name, ItemKind::Runtime, &cuda, download));
    }
    rows.extend(
        items
            .iter()
            .filter(|item| item.kind == ItemKind::Runtime && !is_cuda(item))
            .map(|item| row(item.id.clone(), ItemKind::Runtime, &[item], download)),
    );
    rows
}

/// One row over `items`: on disk when every one is, downloading when the download is at one.
fn row(
    name: String,
    kind: ItemKind,
    items: &[&DownloadItem],
    download: Option<&DownloadProgress>,
) -> ModelRow {
    let bytes: u64 = items.iter().map(|item| item.bytes).sum();
    let present: u64 = items
        .iter()
        .filter(|item| item.present)
        .map(|item| item.bytes)
        .sum();
    let current = download.filter(|d| items.iter().any(|item| item.id == d.id && !item.present));
    let state = match current {
        _ if items.iter().all(|item| item.present) => RowState::OnDisk,
        Some(progress) => {
            RowState::Downloading((present + progress.held) as f32 / bytes.max(1) as f32)
        }
        None => RowState::Missing,
    };
    ModelRow {
        name,
        kind,
        bytes,
        state,
    }
}

/// What is missing of `items`, counted as the Models tab's rows.
pub(crate) fn missing(items: &[DownloadItem]) -> Missing {
    let rows = rows(items, None);
    let count = |kind: ItemKind| {
        rows.iter()
            .filter(|row| row.kind == kind && row.state != RowState::OnDisk)
            .count()
    };
    Missing {
        models: count(ItemKind::Model),
        runtime: count(ItemKind::Runtime),
        bytes: items
            .iter()
            .filter(|item| !item.present)
            .map(|item| item.bytes)
            .sum(),
    }
}

/// The banner `page` shows at `now`, if any: the download while it runs, else what is missing,
/// else for a moment after a download that every item is on disk.
pub(crate) fn banner(page: &SettingsPage, now: Instant) -> Option<Banner> {
    if let Some(progress) = &page.download {
        return Some(Banner::Downloading(progress.clone()));
    }
    let missing = missing(&page.items);
    if missing.any() {
        return Some(Banner::Missing(missing));
    }
    let until = page.downloaded_at? + ALL_ON_DISK_SHOWN;
    (now < until).then_some(Banner::AllOnDisk { until })
}

#[cfg(test)]
#[path = "tests/model_list.rs"]
mod tests;

//! The settings page's state: the file's settings, the owner's unsaved edits, the models a job
//! needs with any download in progress, the machine checks and the work folder's size.

use std::path::PathBuf;

use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::machine::{Check, DownloadItem};

/// A line under the settings form: what the last save or download did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Notice {
    pub(crate) text: String,
    pub(crate) is_error: bool,
}

/// How far a download has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DownloadProgress {
    /// The item being downloaded, by its place in the list.
    pub(crate) index: usize,
    pub(crate) held: u64,
    pub(crate) total: u64,
}

/// Everything the settings view draws.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SettingsPage {
    /// The settings file.
    pub(crate) path: PathBuf,
    /// The settings as saved in the file.
    pub(crate) saved: AppSettings,
    /// The settings as the owner is editing them.
    pub(crate) draft: AppSettings,
    pub(crate) notice: Option<Notice>,
    /// The models and runtime archives the saved settings need.
    pub(crate) items: Vec<DownloadItem>,
    /// `Some` while a download runs.
    pub(crate) download: Option<DownloadProgress>,
    /// `None` until the first check finishes.
    pub(crate) checks: Option<Vec<Check>>,
    pub(crate) checking: bool,
    /// The work folder and its size in bytes, once measured.
    pub(crate) work_folder: PathBuf,
    pub(crate) work_size: Option<u64>,
}

impl SettingsPage {
    /// Whether the draft differs from the file.
    pub(crate) fn has_edits(&self) -> bool {
        self.draft != self.saved
    }
}

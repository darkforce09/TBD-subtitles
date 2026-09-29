//! The settings' state as the Settings window draws it: the file's settings, the error of an edit
//! that was refused, the models a job needs with any download in progress, the machine checks,
//! the sizes of the folders and whether Dolphin's right-click entry is written.
//!
//! **Role:** hold what the five tabs and the models banner show, and name the tabs and the fields
//! an error can sit under.
//!
//! **Position:** built by the application when the window opens; changed by
//! `settings::services::{page_editing, model_downloads}` and the application's settings actions;
//! read by `settings::ui`.
//!
//! **Signals and state:** plain data.
//!
//! **Invariants:** `saved` is always what the settings file holds (or the defaults when it cannot
//! be read); an edit that was refused is never in `saved`, only its `error`.

use std::path::PathBuf;
use std::time::Instant;

use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::machine::{Check, DownloadItem};

/// A tab of the Settings window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsTab {
    General,
    Automation,
    Engines,
    Models,
    ThisComputer,
}

impl SettingsTab {
    pub(crate) const ALL: [SettingsTab; 5] = [
        SettingsTab::General,
        SettingsTab::Automation,
        SettingsTab::Engines,
        SettingsTab::Models,
        SettingsTab::ThisComputer,
    ];

    /// The tab's name in the tab bar.
    pub(crate) fn title(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Automation => "Automation",
            SettingsTab::Engines => "Engines",
            SettingsTab::Models => "Models",
            SettingsTab::ThisComputer => "This Computer",
        }
    }
}

/// A setting the Settings window edits, which an error can sit under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    ModelsFolder,
    WorkFolder,
    OutputFormat,
    Glossary,
    WatchFolders,
    Separator,
    Whisper,
    Model,
    FixModel,
    FixCalls,
    FixAfterRun,
    Processes,
    CutScore,
}

/// Why an edit of `field` was not written: shown in red under the field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FieldError {
    pub(crate) field: Field,
    pub(crate) message: String,
}

/// How far a download has got: the item downloading now, by its id, and the whole download.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DownloadProgress {
    /// The id of the item downloading now.
    pub(crate) id: String,
    /// That item's bytes on disk, and its size.
    pub(crate) held: u64,
    pub(crate) total: u64,
    /// The bytes of the items this download finished, and of every item it downloads.
    pub(crate) finished: u64,
    pub(crate) size: u64,
}

impl DownloadProgress {
    /// The bytes of the whole download on disk.
    pub(crate) fn done(&self) -> u64 {
        (self.finished + self.held).min(self.size)
    }
}

/// Whether "Generate subtitles" is in Dolphin's menu for videos: the service menu the app writes
/// when it is started from its AppImage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) enum RightClickEntry {
    /// No service menu is written: the app was not started from its AppImage.
    #[default]
    NotInstalled,
    /// The service menu at this path.
    Installed(PathBuf),
    /// Why the service menu could not be written.
    Failed(String),
}

/// Everything the Settings window and the models banner draw.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SettingsPage {
    /// The settings file.
    pub(crate) path: PathBuf,
    /// The settings as the file holds them.
    pub(crate) saved: AppSettings,
    /// Why the last edit was not written, until an edit is.
    pub(crate) error: Option<FieldError>,
    /// Why the settings file could not be read, when the defaults show instead.
    pub(crate) unreadable: Option<String>,
    /// The names in the saved glossary, once it has been read.
    pub(crate) glossary_names: Option<usize>,
    /// The models and runtime archives the saved settings need.
    pub(crate) items: Vec<DownloadItem>,
    /// `Some` while a download runs.
    pub(crate) download: Option<DownloadProgress>,
    /// When a download last ended with every item on disk.
    pub(crate) downloaded_at: Option<Instant>,
    /// `None` until the first check finishes.
    pub(crate) checks: Option<Vec<Check>>,
    pub(crate) checking: bool,
    /// The models folder and its size in bytes, once measured.
    pub(crate) models_folder: PathBuf,
    pub(crate) models_size: Option<u64>,
    /// The work folder and its size in bytes, once measured.
    pub(crate) work_folder: PathBuf,
    pub(crate) work_size: Option<u64>,
    /// Whether Dolphin offers "Generate subtitles" for videos.
    pub(crate) right_click: RightClickEntry,
}

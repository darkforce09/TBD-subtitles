//! What the Settings window and the models banner ask the application to do.

use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::page::SettingsTab;

/// A setting that names a path the owner picks in the desktop's chooser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathField {
    ModelsFolder,
    WorkFolder,
    GlossaryFile,
}

/// One request from the Settings window or the models banner.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SettingsEvent {
    /// The owner changed one setting; these are the saved settings with that change, to be
    /// written at once.
    Edit(AppSettings),
    /// Open the Settings window on this tab, or show the tab when it is open.
    Open(SettingsTab),
    /// Open the chooser for a path setting.
    Choose(PathField),
    /// Download every missing model and runtime archive.
    Download,
    StopDownload,
    /// Run the machine checks again.
    CheckAgain,
    /// Open the work folder in the file manager.
    OpenWorkFolder,
}

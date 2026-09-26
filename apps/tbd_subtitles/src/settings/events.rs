//! What the settings view asks the application to do.

use crate::settings::models::app_settings::AppSettings;

/// A setting that names a path the owner picks in the desktop's chooser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PathField {
    ModelsFolder,
    WorkFolder,
    GlossaryFile,
}

/// One request from the settings view.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SettingsEvent {
    /// The owner changed the form; this is the whole draft.
    Edit(AppSettings),
    /// Write the draft to the settings file.
    Save,
    /// Throw the draft away and show the file's settings.
    Revert,
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

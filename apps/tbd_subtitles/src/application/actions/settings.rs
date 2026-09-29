//! The settings' actions: edits written at once, the tab to show, the path choosers, downloads
//! and machine checks, and the threads' answers polled each frame.
//!
//! **Role:** turn each `SettingsEvent` into a change of the settings page or the Settings
//! window's tab, refresh what an edit made stale (the models list, a folder's size), start the
//! thread that does a slow part, and fold the threads' answers back in.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` and before each frame; uses
//! `settings::services` and `core::portal`.
//!
//! **Signals and state:** the settings page, the Settings window's tab, the pending threads and
//! the toasts of the application; writes the settings file through `page_editing`.
//!
//! **Invariants:** one download and one check run at a time; an edit and a chosen path are
//! written at once when `page_editing` allows them, and never otherwise; a settings file that
//! could not be read is kept beside itself before the first write, which a toast says; the cap on
//! Fix It's `claude` calls is the saved "Claude calls at once" after every edit; an edit re-runs
//! no machine check and measures only a folder it changed.

use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;
use std::time::Instant;

use job_model::job::JobSettings;

use crate::application::TbdSubtitlesApp;
use crate::application::background::Chooser;
use crate::application::environment::Environment;
use crate::core::portal::{self, Choose};
use crate::core::toast::ToastKind;
use crate::settings::events::{PathField, SettingsEvent};
use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::page::{RightClickEntry, SettingsPage};
use crate::settings::services::model_downloads::{self, DownloadEnd, Folders};
use crate::settings::services::{
    job_settings, model_list, page_editing, settings_file, system_check, work_folder,
};

/// The settings page as the window opens: the file's settings (the defaults, with the reason,
/// when the file cannot be read), the glossary's names, the models they need, and the machine
/// not yet checked.
pub(crate) fn new_settings_page(env: &Environment) -> SettingsPage {
    let (saved, unreadable) = match settings_file::load(&env.settings_path) {
        Ok(settings) => (settings, None),
        Err(error) => (
            AppSettings::default(),
            Some(format!("{error}; showing the defaults")),
        ),
    };
    let mut page = SettingsPage {
        path: env.settings_path.clone(),
        saved,
        error: None,
        unreadable,
        glossary_names: None,
        items: Vec::new(),
        download: None,
        downloaded_at: None,
        checks: None,
        checking: false,
        models_folder: PathBuf::new(),
        models_size: None,
        work_folder: PathBuf::new(),
        work_size: None,
        right_click: RightClickEntry::NotInstalled,
    };
    page_editing::read_glossary(&mut page);
    refresh_models(&mut page, env);
    page.work_folder = job_settings::work_root(&page.saved).unwrap_or_default();
    page
}

/// The models folder and the models list of the saved settings.
fn refresh_models(page: &mut SettingsPage, env: &Environment) {
    page.models_folder = job_settings::models_dir(&page.saved).unwrap_or_default();
    page.items = model_downloads::plan(&folders(page, env), &engines(&page.saved));
}

fn folders(page: &SettingsPage, env: &Environment) -> Folders {
    Folders {
        models: page.models_folder.clone(),
        runtime: env.runtime_dir.clone(),
        exe_dir: env.exe_dir.clone(),
    }
}

/// The job settings that choose the models, without reading the glossary.
fn engines(settings: &AppSettings) -> JobSettings {
    let mut job = JobSettings::with_glossary(Vec::new());
    job.separator = settings.engines.separator;
    job.whisper = settings.engines.whisper;
    job
}

impl TbdSubtitlesApp {
    /// Start the machine checks and the measures of both folders.
    pub(crate) fn start_settings_threads(&mut self) {
        self.start_checks();
        self.measure(true, true);
    }

    fn start_checks(&mut self) {
        if !self.env.background || self.pending.checks.is_some() {
            return;
        }
        self.settings.checking = true;
        self.pending.checks = Some(system_check::start(
            self.env.exe_dir.clone(),
            self.env.runtime_dir.clone(),
            self.env.wake.clone(),
        ));
    }

    /// Measure the work folder, the models folder, or both, again.
    fn measure(&mut self, work: bool, models: bool) {
        if !self.env.background {
            return;
        }
        let wake = &self.env.wake;
        if work {
            self.settings.work_size = None;
            let folder = self.settings.work_folder.clone();
            self.pending.work_size = Some(work_folder::measure(folder, wake.clone()));
        }
        if models {
            self.settings.models_size = None;
            let folder = self.settings.models_folder.clone();
            self.pending.models_size = Some(work_folder::measure(folder, wake.clone()));
        }
    }

    pub(crate) fn apply_settings(&mut self, event: SettingsEvent) {
        match event {
            SettingsEvent::Edit(edited) => self.edit_settings(edited),
            SettingsEvent::Open(tab) => self.settings_window = Some(tab),
            SettingsEvent::Choose(field) => {
                let (kind, title) = match field {
                    PathField::ModelsFolder => (Choose::Folder, "Models folder"),
                    PathField::WorkFolder => (Choose::Folder, "Work folder"),
                    PathField::GlossaryFile => (Choose::Json, "Glossary: a JSON array of names"),
                    PathField::WatchFolder => (Choose::Folder, "Add a watch folder"),
                };
                self.pending.chooser = Some((
                    Chooser::Setting(field),
                    portal::choose(kind, title, self.env.wake.clone()),
                ));
            }
            SettingsEvent::Download => {
                if self.pending.download.is_none()
                    && let Some(progress) = model_downloads::begun(&self.settings.items)
                {
                    let folders = folders(&self.settings, &self.env);
                    self.pending.download = Some(model_downloads::start(
                        self.settings.items.clone(),
                        folders,
                        self.env.wake.clone(),
                    ));
                    self.settings.download = Some(progress);
                }
            }
            SettingsEvent::StopDownload => {
                if let Some(download) = &self.pending.download {
                    download.stop();
                }
            }
            SettingsEvent::CheckAgain => self.start_checks(),
            SettingsEvent::OpenWorkFolder => {
                // The Settings window says nothing about the answer; a failure is logged.
                drop(portal::open(
                    &self.settings.work_folder,
                    self.env.wake.clone(),
                ));
            }
        }
    }

    /// Write `edited` at once when it can be written, and refresh what it made stale; a settings
    /// file that could not be read is kept beside itself first, which a toast says.
    fn edit_settings(&mut self, edited: AppSettings) {
        let applied = page_editing::apply(&mut self.settings, edited);
        if let Some(kept) = &applied.kept {
            let name = |path: &std::path::Path| {
                path.file_name()
                    .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
            };
            let text = format!(
                "{} could not be read, so it is kept as {}.",
                name(&self.settings.path),
                name(kept)
            );
            self.toast(ToastKind::Info, text);
        }
        // Every Fix It run under way takes the new cap on its next call.
        self.claude_gate
            .set_limit(self.settings.saved.language_model.fix_calls);
        let stale = applied.stale;
        if stale.models {
            refresh_models(&mut self.settings, &self.env);
        }
        if stale.work_size {
            self.settings.work_folder =
                job_settings::work_root(&self.settings.saved).unwrap_or_default();
        }
        self.measure(stale.work_size, stale.models_size);
    }

    /// Put a chosen path into the settings, as an edit; a chosen watch folder is added to the end
    /// of the list unless it is there already.
    pub(crate) fn chosen_setting(&mut self, field: PathField, path: PathBuf) {
        let saved = &self.settings.saved;
        let mut edited = saved.clone();
        match field {
            PathField::ModelsFolder => edited.models_dir = Some(path),
            PathField::WorkFolder => edited.work_root = Some(path),
            PathField::GlossaryFile => edited.glossary = path.to_string_lossy().into_owned(),
            PathField::WatchFolder => edited = page_editing::with_watch_folder(saved, &path),
        }
        self.edit_settings(edited);
    }
}

/// Fold what the settings threads sent into the page.
pub(crate) fn poll_settings(app: &mut TbdSubtitlesApp) {
    if let Some(receiver) = &app.pending.checks {
        match receiver.try_recv() {
            Ok(checks) => {
                app.settings.checks = Some(checks);
                app.settings.checking = false;
                app.pending.checks = None;
            }
            Err(TryRecvError::Disconnected) => {
                app.settings.checking = false;
                app.pending.checks = None;
            }
            Err(TryRecvError::Empty) => {}
        }
    }
    if let Some(receiver) = &app.pending.work_size
        && let Ok(size) = receiver.try_recv()
    {
        app.settings.work_size = Some(size);
        app.pending.work_size = None;
    }
    if let Some(receiver) = &app.pending.models_size
        && let Ok(size) = receiver.try_recv()
    {
        app.settings.models_size = Some(size);
        app.pending.models_size = None;
    }
    let mut ended = None;
    if let Some(download) = &app.pending.download {
        while let Ok(event) = download.events.try_recv() {
            if let Some(end) = model_downloads::fold(&mut app.settings, event) {
                ended = Some(end);
            }
        }
    }
    if let Some(end) = ended {
        download_ended(app, end);
    }
}

/// A download ended: list the models again, and say how it ended; a download that brought
/// everything onto disk says so in the banner for a moment, and the checks and the models
/// folder's size are taken again.
fn download_ended(app: &mut TbdSubtitlesApp, end: DownloadEnd) {
    app.pending.download = None;
    app.settings.download = None;
    refresh_models(&mut app.settings, &app.env);
    match end {
        DownloadEnd::Done if !model_list::missing(&app.settings.items).any() => {
            app.settings.downloaded_at = Some(Instant::now());
        }
        DownloadEnd::Done => {}
        DownloadEnd::Stopped => app.toast(
            ToastKind::Info,
            "Download stopped. It resumes where it left off.",
        ),
        DownloadEnd::Failed(reason) => {
            app.toast(ToastKind::Error, format!("The download failed: {reason}"));
        }
    }
    app.start_checks();
    app.measure(false, true);
}

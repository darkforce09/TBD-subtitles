//! The settings page's actions: edits, saving, the path choosers, downloads and machine checks,
//! and the threads' answers polled each frame.
//!
//! **Role:** turn each `SettingsEvent` into a change of the settings page, starting the thread
//! that does the slow part, and fold the threads' answers back in.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` and before each frame; uses
//! `settings::services` and `core::portal`.
//!
//! **Signals and state:** the settings page and the pending threads of the application.
//!
//! **Invariants:** one download and one check run at a time; a chooser answer changes only the
//! draft, never the file.

use std::path::PathBuf;
use std::sync::mpsc::TryRecvError;

use job_model::job::JobSettings;

use crate::application::TbdSubtitlesApp;
use crate::application::background::Chooser;
use crate::application::environment::Environment;
use crate::core::portal::{self, Choose};
use crate::settings::events::{PathField, SettingsEvent};
use crate::settings::models::app_settings::AppSettings;
use crate::settings::models::page::{DownloadProgress, Notice, SettingsPage};
use crate::settings::services::model_downloads::{self, DownloadEvent, Folders};
use crate::settings::services::{
    job_settings, page_editing, settings_file, system_check, work_folder,
};

/// The settings page as the window opens: the file's settings (the defaults, with the reason,
/// when the file cannot be read), the models they need, and the machine not yet checked.
pub(crate) fn new_settings_page(env: &Environment) -> SettingsPage {
    let (saved, notice) = match settings_file::load(&env.settings_path) {
        Ok(settings) => (settings, None),
        Err(error) => (
            AppSettings::default(),
            Some(Notice {
                text: format!("{error}; showing the defaults"),
                is_error: true,
            }),
        ),
    };
    let mut page = SettingsPage {
        path: env.settings_path.clone(),
        draft: saved.clone(),
        saved,
        notice,
        items: Vec::new(),
        download: None,
        checks: None,
        checking: false,
        work_folder: PathBuf::new(),
        work_size: None,
    };
    refresh_folders(&mut page, env);
    page
}

/// The folders and the model list of the saved settings.
fn refresh_folders(page: &mut SettingsPage, env: &Environment) {
    page.work_folder = job_settings::work_root(&page.saved).unwrap_or_default();
    page.items = model_downloads::plan(&folders(page, env), &engines(&page.saved));
}

fn folders(page: &SettingsPage, env: &Environment) -> Folders {
    Folders {
        models: job_settings::models_dir(&page.saved).unwrap_or_default(),
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
    /// Start the machine checks and the work folder's measure.
    pub(crate) fn start_settings_threads(&mut self) {
        if !self.env.background {
            return;
        }
        self.start_checks();
        self.pending.work_size = Some(work_folder::measure(
            self.settings.work_folder.clone(),
            self.env.wake.clone(),
        ));
    }

    fn start_checks(&mut self) {
        self.settings.checking = true;
        self.pending.checks = Some(system_check::start(
            self.env.exe_dir.clone(),
            self.env.runtime_dir.clone(),
            self.env.wake.clone(),
        ));
    }

    pub(crate) fn apply_settings(&mut self, event: SettingsEvent) {
        match event {
            SettingsEvent::Edit(draft) => self.settings.draft = draft,
            SettingsEvent::Save => {
                page_editing::save(&mut self.settings);
                refresh_folders(&mut self.settings, &self.env);
                self.start_settings_threads();
            }
            SettingsEvent::Revert => page_editing::revert(&mut self.settings),
            SettingsEvent::Choose(field) => {
                let (kind, title) = match field {
                    PathField::ModelsFolder => (Choose::Folder, "Models folder"),
                    PathField::WorkFolder => (Choose::Folder, "Work folder"),
                    PathField::GlossaryFile => (Choose::Json, "Glossary: a JSON array of names"),
                };
                self.pending.chooser = Some((
                    Chooser::Setting(field),
                    portal::choose(kind, title, self.env.wake.clone()),
                ));
            }
            SettingsEvent::Download => {
                if self.pending.download.is_none() {
                    let folders = folders(&self.settings, &self.env);
                    self.pending.download = Some(model_downloads::start(
                        self.settings.items.clone(),
                        folders,
                        self.env.wake.clone(),
                    ));
                    self.settings.notice = None;
                }
            }
            SettingsEvent::StopDownload => {
                if let Some(download) = &self.pending.download {
                    download.stop();
                }
            }
            SettingsEvent::CheckAgain => {
                if self.pending.checks.is_none() {
                    self.start_checks();
                }
            }
            SettingsEvent::OpenWorkFolder => portal::open(&self.settings.work_folder),
        }
    }

    /// Put a chosen path into the draft.
    pub(crate) fn chosen_setting(&mut self, field: PathField, path: PathBuf) {
        let draft = &mut self.settings.draft;
        match field {
            PathField::ModelsFolder => draft.models_dir = Some(path),
            PathField::WorkFolder => draft.work_root = Some(path),
            PathField::GlossaryFile => draft.glossary = path.to_string_lossy().into_owned(),
        }
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
    let mut ended = None;
    if let Some(download) = &app.pending.download {
        while let Ok(event) = download.events.try_recv() {
            match event {
                DownloadEvent::Advanced { index, held, total } => {
                    app.settings.download = Some(DownloadProgress { index, held, total });
                }
                DownloadEvent::Finished { index } => {
                    if let Some(item) = app.settings.items.get_mut(index) {
                        item.present = true;
                    }
                }
                DownloadEvent::Ended(result) => ended = Some(result),
            }
        }
    }
    if let Some(result) = ended {
        app.pending.download = None;
        app.settings.download = None;
        app.settings.notice = Some(match result {
            Ok(()) => Notice {
                text: "Every model and runtime archive is on disk.".to_string(),
                is_error: false,
            },
            Err(reason) => Notice {
                text: format!("Download ended: {reason}"),
                is_error: true,
            },
        });
        refresh_folders(&mut app.settings, &app.env);
    }
}

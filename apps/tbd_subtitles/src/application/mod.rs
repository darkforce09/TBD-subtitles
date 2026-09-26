//! The desktop window: its state, the frame, and the actions that change the state.
//!
//! **Role:** owns the application state, lends each feature a borrowed view every frame, collects
//! the events the features return, and applies them as actions after the frame.
//!
//! **Position:** started by `cli` for the `gui` subcommand; draws the features' `ui` modules and
//! changes state through their `services`. No feature imports this module.
//!
//! **Signals and state:** holds the queue, the page shown, the settings page and the threads it
//! waits on; reads files dropped onto the window.
//!
//! **Invariants:** nothing changes state while a frame is drawn: every change is an [`Action`]
//! applied after the frame, or a thread's answer folded in before it.

mod actions;
mod background;
mod environment;
mod events;
mod feature_views;
mod window;

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::anyhow;
use eframe::egui;

pub(crate) use environment::Environment;
pub(crate) use events::{Action, Page};

use pipeline::CancelToken;

use crate::job_queue::models::progress::Rates;
use crate::job_queue::models::queue::{JobId, Queue};
use crate::job_queue::services::job_runner::{self, JobRunner};
use crate::job_queue::services::{queue_editing, queue_store, time_left};
use crate::job_report::events::ReportEvent;
use crate::job_report::models::report::JobReport;
use crate::settings::models::page::SettingsPage;
use crate::settings::services::job_settings;

/// The window's name, title and desktop application id.
const APP_NAME: &str = "TBD Subtitles";
const APP_ID: &str = "tbd-subtitles";

/// The application state.
pub(crate) struct TbdSubtitlesApp {
    env: Environment,
    page: Page,
    /// Every job, in run order.
    queue: Queue,
    /// The thread that runs the jobs.
    runner: JobRunner,
    /// The running job and the token that stops it.
    cancel: Option<(JobId, CancelToken)>,
    /// Each step's seconds per second of video, for the time left.
    rates: Rates,
    settings: SettingsPage,
    pending: background::Pending,
    /// The selected finished job's report, read from its work directory.
    report: Option<(JobId, Result<JobReport, String>)>,
}

impl TbdSubtitlesApp {
    fn new(env: Environment, videos: Vec<PathBuf>) -> TbdSubtitlesApp {
        let settings = actions::new_settings_page(&env);
        let queue = queue_store::load(&env.queue_path).unwrap_or_else(|error| {
            tracing::warn!(%error, "starting with an empty queue");
            Queue::default()
        });
        let rates = job_settings::work_root(&settings.saved)
            .map(|root| time_left::from_history(&root))
            .unwrap_or_else(|_| time_left::pilot_rates());
        let runner = job_runner::start(env.run_job.clone(), env.wake.clone());
        let mut app = TbdSubtitlesApp {
            env,
            page: Page::Jobs,
            queue,
            runner,
            cancel: None,
            rates,
            settings,
            pending: background::Pending::default(),
            report: None,
        };
        app.start_settings_threads();
        app.apply(vec![Action::QueueVideos(videos)]);
        app
    }

    /// Apply the actions collected during a frame, in order.
    fn apply(&mut self, actions: Vec<Action>) {
        for action in actions {
            match action {
                Action::QueueVideos(videos) => {
                    let added = queue_editing::add_videos(&mut self.queue, videos);
                    if added > 0 {
                        tracing::info!(added, "videos queued");
                        self.save_queue();
                    }
                }
                Action::Queue(event) => self.apply_queue(event),
                Action::ShowPage(page) => self.page = page,
                Action::Settings(event) => self.apply_settings(event),
                Action::Report(ReportEvent::Open(path)) => crate::core::portal::open(&path),
            }
        }
    }
}

/// Open the window with `videos` in the queue, and return when it closes.
pub(crate) fn launch(videos: Vec<PathBuf>) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id(APP_ID)
            .with_inner_size([1200.0, 760.0])
            .with_min_inner_size([760.0, 480.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |creation| {
            let context = creation.egui_ctx.clone();
            let env = Environment::real(Arc::new(move || context.request_repaint()))?;
            Ok(Box::new(TbdSubtitlesApp::new(env, videos)))
        }),
    )
    .map_err(|error| anyhow!("the window could not open: {error}"))
}

#[cfg(test)]
#[path = "tests/rendering.rs"]
mod tests;

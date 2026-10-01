//! The desktop window: its state, the frame, and the actions that change the state.
//!
//! **Role:** owns the application state, lends each feature a borrowed view every frame, collects
//! the events the features return, and applies them as actions after the frame.
//!
//! **Position:** started by `cli` for videos alone, `gui` and `process --enqueue`; draws the
//! features' `ui` modules and changes state through their `services`. No feature imports this
//! module.
//!
//! **Signals and state:** holds the queue, the toasts, the row removed last, the Settings window's
//! tab while it is open, whether the log window is open with its lines and filter, the settings
//! page, the desktop's colour scheme, the finished rows' summaries, the open line review with its
//! clip and still frame, the open Check Text with its saves and preview, the edits of closed
//! reviews, the one cap on Fix It's `claude` calls, the Fix It runs waiting for their correction
//! run and the video fixed last, whether the window is away, the later starts' hand-offs, the
//! watch folders' watcher, every video ever queued, whether the owner paused the queue, and the
//! threads it waits on; reads files dropped onto the window; writes the Dolphin service menu as
//! it opens; logs each action it applies, but the log window's own.
//!
//! **Invariants:** nothing changes state while a frame is drawn: every change is an [`Action`]
//! applied after the frame, or a thread's answer folded in before it; the threads' answers are
//! folded in by `App::logic`, which runs while the window is minimized too.

mod actions;
mod background;
mod detail_view;
mod environment;
mod events;
mod feature_views;
mod log_window;
mod settings_window;
mod shortcuts;
mod window;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, OnceLock};

use anyhow::anyhow;
use eframe::egui;
use inference::llm::call_gate::CallGate;

use detail_view::DetailTab;
pub(crate) use environment::Environment;
pub(crate) use events::Action;

use pipeline::CancelToken;

use crate::core::background::Wake;
use crate::core::color_scheme::Scheme;
use crate::core::single_instance::HandOff;
use crate::core::toast::Toasts;
use crate::core::ui::theme;
use crate::job_queue::models::progress::Rates;
use crate::job_queue::models::queue::{JobId, Queue, Removed};
use crate::job_queue::services::folder_watcher::{self, FolderWatcher};
use crate::job_queue::services::job_runner::{self, JobRunner};
use crate::job_queue::services::queued_history::QueuedHistory;
use crate::job_queue::services::review_lanes::ReviewLanes;
use crate::job_queue::services::{queue_store, time_left};
use crate::job_report::models::report::JobReport;
use crate::job_report::models::summary::RowSummary;
use crate::line_review::events::ReviewEvent;
use crate::line_review::models::session::{Parked, ReviewSession};
use crate::line_review::services::clip_player::{ClipPlayer, Still};
use crate::log_console::models::console::LogConsole;
use crate::settings::models::page::{SettingsPage, SettingsTab};
use crate::settings::services::job_settings;

/// The window's name, title and desktop application id.
const APP_NAME: &str = "TBD Subtitles";
const APP_ID: &str = "tbd-subtitles";
/// The side of the window's icon, in pixels.
const ICON_SIZE: u32 = 256;

/// The application state.
pub(crate) struct TbdSubtitlesApp {
    env: Environment,
    /// Every job, in run order.
    queue: Queue,
    /// The thread that runs the full jobs, one at a time.
    runner: JobRunner,
    /// The running full job and the token that stops it.
    cancel: Option<(JobId, CancelToken)>,
    /// The threads that run correction runs beside a full job, up to four of different videos at
    /// once: their one stale model step runs on the CPU. Each holds its running run and the
    /// token that stops it.
    review_lanes: ReviewLanes,
    /// Each step's seconds per second of video, for the time left.
    rates: Rates,
    settings: SettingsPage,
    /// The Settings window's tab while it is open; `None` while it is closed.
    settings_window: Option<SettingsTab>,
    /// Whether the log window is open.
    log_window: bool,
    /// The log window.s lines and filter.
    console: LogConsole,
    /// The short messages at the bottom of the window; a button's action is applied as is.
    toasts: Toasts<Action>,
    /// The row removed last, which Undo puts back; a newer removal replaces it.
    removed: Option<Removed>,
    /// The desktop's colour scheme, which the window follows.
    scheme: Scheme,
    pending: background::Pending,
    /// The selected finished job's report, read from its work directory.
    report: Option<(JobId, Result<JobReport, String>)>,
    /// Each finished job's verdict and lines to check, for its sidebar row and header: read when
    /// the window opens and after each run and correction of its video.
    summaries: HashMap<JobId, RowSummary>,
    /// The selected job's line review, while it is open.
    review: Option<(JobId, ReviewSession)>,
    text: actions::text::State,
    /// The clip playing in the review.
    clip: Option<ClipPlayer>,
    /// The line the review's editor shows, with its still frame when its video has a picture.
    still: Option<(String, Option<Still>)>,
    /// The unsaved line edits and the run states of each job whose review closed, until it opens
    /// again or the window closes.
    parked: HashMap<JobId, Parked>,
    /// The one cap on the `claude` calls of every Fix It run together, the saved "Claude calls
    /// at once"; an edit of the setting changes it at once.
    claude_gate: Arc<CallGate>,
    /// Each video whose Fix It run ended, until its changes are in the subtitles.
    fix_followups: HashMap<PathBuf, actions::FixFollowup>,
    /// The video Fix It finished last, with egui's time then, until another job is selected.
    just_fixed: Option<(PathBuf, f64)>,
    /// egui's clock and whether the window is away, as the last frame saw them.
    presence: window::Presence,
    /// Whether to ask the desktop for the window's attention at the next frame.
    attention: bool,
    /// The hand-offs of later starts, when this window holds the single instance.
    hand_offs: Option<Receiver<HandOff>>,
    /// The thread that watches the saved watch folders.
    watcher: FolderWatcher,
    /// Every video ever queued, from any source, so a watch folder never queues one twice.
    history: QueuedHistory,
    /// Whether the owner pressed Pause in this window, which keeps automation from starting the
    /// queue until the owner presses Start.
    paused_by_owner: bool,
    /// Whether to bring the window forward at the next frame.
    raise: bool,
    /// Whether to minimize the window at the next frame, once, as a start that queues asks.
    minimize_once: bool,
}

/// The hand-offs of later starts, and the cell the window puts its wake in so that each wakes it.
pub(crate) type HandOffs = (Receiver<HandOff>, Arc<OnceLock<Wake>>);

/// How the window opens.
pub(crate) struct Launch {
    /// The videos to queue.
    pub(crate) videos: Vec<PathBuf>,
    /// Whether the queue starts once they are queued.
    pub(crate) start: bool,
    /// Whether the window minimizes as it opens.
    pub(crate) minimized: bool,
    /// The later starts' hand-offs, when this window holds the single instance.
    pub(crate) hand_offs: Option<HandOffs>,
}

impl TbdSubtitlesApp {
    fn new(env: Environment, videos: Vec<PathBuf>) -> TbdSubtitlesApp {
        let settings = actions::new_settings_page(&env);
        let mut queue = queue_store::load(&env.queue_path).unwrap_or_else(|error| {
            tracing::warn!(%error, "starting with an empty queue");
            Queue::default()
        });
        let work_root = job_settings::work_root(&settings.saved).ok();
        if let Some(root) = &work_root {
            queue_store::read_finished_steps(&mut queue, root);
        }
        let rates = work_root.map_or_else(time_left::pilot_rates, |root| {
            time_left::from_history(&root)
        });
        let runner = job_runner::start("job-runner", env.run_job.clone(), env.wake.clone());
        let review_lanes = ReviewLanes::start(env.run_job.clone(), env.wake.clone());
        let claude_gate = CallGate::new(settings.saved.language_model.fix_calls);
        if let Err(error) =
            inference::llm::claude_cli::set_shared_limit(settings.saved.language_model.fix_calls)
        {
            tracing::warn!(%error, "Cannot configure the shared Claude call cap");
        }
        let history = actions::seeded_history(&env.history_path, &queue);
        let watcher = folder_watcher::start(
            settings.saved.watch_folders.clone(),
            folder_watcher::WATCH_INTERVAL,
            env.wake.clone(),
        );
        let mut app = TbdSubtitlesApp {
            env,
            queue,
            runner,
            cancel: None,
            review_lanes,
            rates,
            settings,
            settings_window: None,
            log_window: false,
            console: LogConsole::default(),
            toasts: Toasts::default(),
            removed: None,
            scheme: Scheme::default(),
            pending: background::Pending::default(),
            report: None,
            summaries: HashMap::new(),
            review: None,
            text: actions::text::State::default(),
            clip: None,
            still: None,
            parked: HashMap::new(),
            claude_gate,
            fix_followups: HashMap::new(),
            just_fixed: None,
            presence: window::Presence::default(),
            attention: false,
            hand_offs: None,
            watcher,
            history,
            paused_by_owner: false,
            raise: false,
            minimize_once: false,
        };
        app.refresh_summaries(None);
        app.watch_scheme();
        app.start_settings_threads();
        app.apply(vec![Action::QueueVideos(videos)]);
        app
    }

    /// Apply the actions collected during a frame, in order.
    fn apply(&mut self, actions: Vec<Action>) {
        for action in actions {
            if !matches!(action, Action::LogConsole(_)) {
                tracing::debug!("action {}", action.describe());
            }
            match action {
                Action::QueueVideos(videos) => {
                    self.queue_videos(videos);
                }
                Action::Queue(event) => self.apply_queue(event),
                Action::ShowTab(tab) => self.show_tab(tab),
                Action::ShowSettings(true) => {
                    self.settings_window.get_or_insert(SettingsTab::General);
                }
                Action::ShowSettings(false) => self.settings_window = None,
                Action::ShowLog(open) => self.show_log(open),
                Action::ToastButton(id) => {
                    if let Some((_, action)) = self.toasts.take(id).and_then(|toast| toast.action) {
                        self.apply(vec![action]);
                    }
                }
                Action::SeeFixChanges(id) => self.see_fix_changes(id),
                Action::FixIt(id) => self.start_fix(id),
                Action::StopFix(id) => self.stop_fix(id),
                Action::Settings(event) => self.apply_settings(event),
                Action::Report(event) => self.apply_report(event),
                Action::Review(event) => self.apply_review(event),
                Action::Text(event) => self.apply_text(event),
                Action::LogConsole(event) => self.apply_log_console(event),
            }
        }
    }

    /// Show the selected finished job's report, closing its line review, or open its lines to
    /// check.
    fn show_tab(&mut self, tab: DetailTab) {
        let Some(id) = self.queue.selected else {
            return;
        };
        if tab != DetailTab::CheckText {
            self.close_text();
        }
        match tab {
            DetailTab::Overview => self.apply_review(ReviewEvent::Close),
            DetailTab::CheckLines if self.detail_tab(id) != DetailTab::CheckLines => {
                self.open_review(None);
            }
            DetailTab::CheckLines => {}
            DetailTab::CheckText => self.open_text(id),
        }
    }
}

/// Open the window as `how` asks, and return when it closes.
///
/// The window runs under X11 (XWayland on a Wayland desktop): only there can files be dropped
/// onto it and Settings and the log be placed over it.
pub(crate) fn launch(how: Launch) -> anyhow::Result<()> {
    let right_click = actions::install_right_click();
    let icon = egui::IconData {
        rgba: app_icon::rgba(ICON_SIZE),
        width: ICON_SIZE,
        height: ICON_SIZE,
    };
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_app_id(APP_ID)
            .with_icon(Arc::new(icon))
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([1100.0, 700.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        event_loop_builder: Some(Box::new(|builder| {
            use winit::platform::x11::EventLoopBuilderExtX11 as _;
            builder.with_x11();
        })),
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |creation| {
            theme::install(&creation.egui_ctx);
            let context = creation.egui_ctx.clone();
            let env = Environment::real(Arc::new(move || context.request_repaint()))?;
            let mut app = TbdSubtitlesApp::new(env, how.videos);
            app.settings.right_click = right_click;
            app.automate(how.start, how.minimized, how.hand_offs);
            theme::follow(&creation.egui_ctx, app.scheme);
            Ok(Box::new(app))
        }),
    )
    .map_err(|error| anyhow!("the window could not open: {error}"))
}

#[cfg(test)]
#[path = "tests/rendering.rs"]
mod tests;

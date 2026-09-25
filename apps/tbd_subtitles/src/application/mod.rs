//! The desktop window: its state, the frame, and the actions that change the state.
//!
//! **Role:** owns the application state, lends each feature a borrowed view every frame, collects
//! the events the features return, and applies them as actions after the frame.
//!
//! **Position:** started by `cli` for the `gui` subcommand; draws the features' `ui` modules and
//! changes state through their `services`. No feature imports this module.
//!
//! **Signals and state:** holds the queue; reads files dropped onto the window.
//!
//! **Invariants:** nothing changes state while a frame is drawn: every change is an [`Action`]
//! applied after the frame.

mod events;
mod feature_views;
mod window;

use std::path::PathBuf;

use anyhow::anyhow;
use eframe::egui;

pub(crate) use events::Action;

/// The window's name, title and desktop application id.
const APP_NAME: &str = "TBD Subtitles";
const APP_ID: &str = "tbd-subtitles";

/// The application state.
pub(crate) struct TbdSubtitlesApp {
    /// Videos waiting for subtitles, in run order.
    queue: Vec<PathBuf>,
}

impl TbdSubtitlesApp {
    fn new(videos: Vec<PathBuf>) -> TbdSubtitlesApp {
        let mut app = TbdSubtitlesApp { queue: Vec::new() };
        app.apply(vec![Action::QueueVideos(videos)]);
        app
    }

    /// Apply the actions collected during a frame, in order.
    fn apply(&mut self, actions: Vec<Action>) {
        use crate::job_queue::services::queue_editing;
        for action in actions {
            match action {
                Action::QueueVideos(videos) => {
                    let added = queue_editing::add_videos(&mut self.queue, videos);
                    tracing::info!(added, queued = self.queue.len(), "videos queued");
                }
                Action::RemoveFromQueue(index) => {
                    if let Some(video) = queue_editing::remove_video(&mut self.queue, index) {
                        tracing::info!(video = %video.display(), "video removed from the queue");
                    }
                }
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
            .with_inner_size([1100.0, 700.0])
            .with_min_inner_size([640.0, 400.0])
            .with_drag_and_drop(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        APP_NAME,
        options,
        Box::new(move |_creation| Ok(Box::new(TbdSubtitlesApp::new(videos)))),
    )
    .map_err(|error| anyhow!("the window could not open: {error}"))
}

#[cfg(test)]
#[path = "tests/rendering.rs"]
mod tests;

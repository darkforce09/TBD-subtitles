//! The starts that concern the window: videos alone, `gui`, and `process --enqueue`.
//!
//! **Role:** claim the single instance; the first start opens the window, with the hand-offs of
//! later starts coming to it, and a later start hands its videos to that window and exits.
//!
//! **Position:** called by `cli::run` before logging starts; uses `core::single_instance` and
//! `application::launch`; `process_command::expand` finds the videos in a folder given to
//! `--enqueue`.
//!
//! **Signals and state:** takes the instance lock and socket in `single_instance::instance_dir()`
//! for as long as the window is open; starts logging: the window's log file for the window that
//! opens, stderr only for a start that hands off.
//!
//! **Invariants:** a start never opens a second window while one is open; a claim that fails
//! with an error opens the window without the single instance rather than none; `--enqueue`
//! starts the queue and opens the window minimized, videos alone and `gui` raise it instead.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::anyhow;

use crate::application::{HandOffs, Launch};
use crate::core::logging::{self, LogRun};
use crate::core::single_instance::{self, Claim, HandOff};

use super::process_command;

/// How long a later start waits for the window already open to take its videos.
const HAND_OFF_PATIENCE: Duration = Duration::from_secs(5);

/// What a start asks of the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WindowRequest {
    /// The videos, or with `enqueue` the videos and folders, to queue.
    pub(super) videos: Vec<PathBuf>,
    /// Whether the queue starts at once, with the window minimized when it opens.
    pub(super) enqueue: bool,
}

impl WindowRequest {
    /// Open the window, or raise the one already open, with `videos` queued.
    pub(super) fn open(videos: Vec<PathBuf>) -> WindowRequest {
        WindowRequest {
            videos,
            enqueue: false,
        }
    }

    /// Queue `paths` in the window and start the queue, without bringing the window forward.
    pub(super) fn enqueue(paths: Vec<PathBuf>) -> WindowRequest {
        WindowRequest {
            videos: paths,
            enqueue: true,
        }
    }

    /// The message a later start sends the window already open.
    pub(super) fn hand_off(&self) -> HandOff {
        HandOff {
            videos: self.videos.clone(),
            start: self.enqueue,
            raise: !self.enqueue,
        }
    }

    /// How the window opens when this start is the first.
    pub(super) fn launch(self, hand_offs: Option<HandOffs>) -> Launch {
        Launch {
            videos: self.videos,
            start: self.enqueue,
            minimized: self.enqueue,
            hand_offs,
        }
    }
}

/// Open the window for `request`, or hand `request` to the window already open.
pub(super) fn run(mut request: WindowRequest) -> anyhow::Result<ExitCode> {
    if request.enqueue {
        // A folder is searched as `process` searches it; a path that is neither fails here.
        let expanded = process_command::expand(&request.videos);
        match expanded {
            Ok(videos) => request.videos = videos,
            Err(error) => {
                logging::initialise(LogRun::Command);
                return Err(error);
            }
        }
    }
    let claimed = single_instance::instance_dir().and_then(|dir| {
        let claim = single_instance::claim(&dir)
            .map_err(|error| anyhow!("claiming the instance in {}: {error}", dir.display()))?;
        Ok((dir, claim))
    });
    match claimed {
        Ok((dir, Claim::Running)) => {
            logging::initialise(LogRun::Command);
            single_instance::hand_off(&dir, &request.hand_off(), HAND_OFF_PATIENCE)
                .map_err(|error| anyhow!("the open window did not take the videos: {error}"))?;
            tracing::info!(
                videos = request.videos.len(),
                "handed the videos to the open window"
            );
            Ok(ExitCode::SUCCESS)
        }
        Ok((_, Claim::First(instance))) => {
            logging::initialise(LogRun::Window);
            let wake = Arc::new(OnceLock::new());
            let receiver = single_instance::serve(instance, wake.clone());
            crate::application::launch(request.launch(Some((receiver, wake))))?;
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            logging::initialise(LogRun::Window);
            tracing::warn!(
                error = format!("{error:#}"),
                "opening the window without the single instance"
            );
            crate::application::launch(request.launch(None))?;
            Ok(ExitCode::SUCCESS)
        }
    }
}

//! The threads the window waits on, and folding their answers in before each frame.
//!
//! **Role:** hold the receiving end of every thread the application started (the desktop's
//! chooser, the files the desktop was asked to open, the model download, the machine checks, the
//! sizes of the work and models folders, the desktop's colour scheme, the Fix It runs) and apply
//! what they sent;
//! let the toasts whose time is up go.
//!
//! **Position:** owned by `TbdSubtitlesApp`; polled by `window` before each frame; the settings
//! part lives in `actions::settings`.
//!
//! **Signals and state:** channels only; each thread wakes the window after it sends.
//!
//! **Invariants:** at most one chooser is open at a time, and a second ask says so; a chooser or
//! an open that fails says so in a red toast, and so does an open the desktop has not answered
//! within [`OPEN_DEADLINE`]; a closed channel ends the wait.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use super::TbdSubtitlesApp;
use super::actions::{poll_fix, poll_log, poll_runner, poll_settings};
use crate::core::color_scheme::{self, Scheme};
use crate::core::portal::{self, Choose, Chosen, Opened};
use crate::core::toast::ToastKind;
use crate::job_queue::models::queue::JobId;
use crate::job_report::services::fix_it::Fixing;
use crate::settings::events::PathField;
use crate::settings::models::machine::Check;
use crate::settings::services::model_downloads::Downloading;

/// How long the window waits for the desktop's colour scheme before its first frame; the portal
/// answers in a few milliseconds.
const FIRST_SCHEME: Duration = Duration::from_millis(250);

/// How long the window waits for the desktop to answer an open before it says the desktop did
/// not; the portal answers once the program has started, in a second or two.
pub(crate) const OPEN_DEADLINE: Duration = Duration::from_secs(20);

/// What a chooser's answer is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chooser {
    Setting(PathField),
    QueueVideos,
    QueueFolder,
}

/// What the desktop is asked to do with a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Opening {
    /// Play a video in the desktop's video player.
    Play,
    /// Show a file in the file manager, in its folder.
    Reveal,
    /// Open a text file in the desktop's text editor.
    Read,
}

/// The threads the application waits on.
#[derive(Default)]
pub(crate) struct Pending {
    pub(crate) chooser: Option<(Chooser, Receiver<Chosen>)>,
    pub(crate) download: Option<Downloading>,
    pub(crate) checks: Option<Receiver<Vec<Check>>>,
    pub(crate) work_size: Option<Receiver<u64>>,
    pub(crate) models_size: Option<Receiver<u64>>,
    pub(crate) scheme: Option<Receiver<Scheme>>,
    /// The desktop's answers to the files it was asked to open.
    pub(crate) opens: Vec<OpenRequest>,
    /// Every Fix It run under way, by the job it fixes; at most one per video.
    pub(crate) fixes: BTreeMap<JobId, Fixing>,
}

/// A file the desktop was asked to open, waiting for its answer.
pub(crate) struct OpenRequest {
    /// The words a failure toast starts with.
    pub(crate) failed: String,
    /// When the desktop was asked.
    pub(crate) asked: Instant,
    pub(crate) answer: Receiver<Opened>,
}

impl Pending {
    /// When the oldest unanswered open runs out of time, so the window can wake to say so.
    pub(crate) fn next_open_deadline(&self) -> Option<Instant> {
        self.opens
            .iter()
            .map(|open| open.asked + OPEN_DEADLINE)
            .min()
    }
}

impl TbdSubtitlesApp {
    /// Apply everything the threads sent since the last frame.
    pub(crate) fn poll(&mut self) {
        poll_settings(self);
        poll_runner(self);
        poll_fix(self);
        poll_log(self);
        self.poll_chooser();
        self.poll_scheme();
        self.poll_opens(Instant::now());
        self.toasts.expire(Instant::now());
    }

    /// Ask the desktop to do `how` with `path`, saying so in a toast; a failure comes back as a
    /// red toast.
    pub(crate) fn open_with_desktop(&mut self, how: Opening, path: &Path) {
        let name = path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
        let wake = self.env.wake.clone();
        let (answer, doing, failed) = match how {
            Opening::Play => (
                portal::open(path, wake),
                format!("Opening {name} in your video player."),
                format!("{name} could not open"),
            ),
            Opening::Reveal => (
                portal::reveal(path, wake),
                format!("Showing {name} in your file manager."),
                format!("The file manager could not show {name}"),
            ),
            Opening::Read => (
                portal::open(path, wake),
                format!("Opening {name} in your text editor."),
                format!("{name} could not open"),
            ),
        };
        self.toast(ToastKind::Info, doing);
        self.pending.opens.push(OpenRequest {
            failed,
            asked: Instant::now(),
            answer,
        });
    }

    /// Say in a red toast when the desktop could not open a file, or has not answered by `now`
    /// within [`OPEN_DEADLINE`]; forget the answered and the timed-out requests.
    pub(crate) fn poll_opens(&mut self, now: Instant) {
        let mut failures = Vec::new();
        self.pending
            .opens
            .retain(|open| match open.answer.try_recv() {
                Ok(Ok(())) | Err(TryRecvError::Disconnected) => false,
                Ok(Err(error)) => {
                    failures.push(format!("{}: {error}", open.failed));
                    false
                }
                Err(TryRecvError::Empty) if now >= open.asked + OPEN_DEADLINE => {
                    tracing::warn!(failed = %open.failed, "the desktop did not answer an open");
                    failures.push(format!("{}: the desktop did not answer", open.failed));
                    false
                }
                Err(TryRecvError::Empty) => true,
            });
        for failure in failures {
            self.toast(ToastKind::Error, failure);
        }
    }

    /// Follow the desktop's colour scheme, waiting briefly for its first answer so the first
    /// frame already has the desktop's colours; nothing starts in the tests.
    pub(crate) fn watch_scheme(&mut self) {
        if !self.env.background {
            return;
        }
        let schemes = color_scheme::watch(self.env.wake.clone());
        if let Ok(scheme) = schemes.recv_timeout(FIRST_SCHEME) {
            self.scheme = scheme;
        }
        self.pending.scheme = Some(schemes);
    }

    fn poll_scheme(&mut self) {
        let Some(schemes) = &self.pending.scheme else {
            return;
        };
        loop {
            match schemes.try_recv() {
                Ok(scheme) => self.scheme = scheme,
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        self.pending.scheme = None;
    }

    fn poll_chooser(&mut self) {
        let Some((purpose, receiver)) = &self.pending.chooser else {
            return;
        };
        let purpose = *purpose;
        let answer = match receiver.try_recv() {
            Ok(answer) => answer,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err("the chooser closed".to_string()),
        };
        self.pending.chooser = None;
        match answer {
            Ok(paths) => self.chosen(purpose, paths),
            Err(error) => {
                tracing::warn!(%error, "the desktop's chooser failed");
                self.toast(
                    ToastKind::Error,
                    format!("The desktop's file chooser failed: {error}"),
                );
            }
        }
    }

    /// Open the desktop's chooser for videos or a folder; its answer is queued.
    pub(crate) fn choose_for_queue(&mut self, folder: bool) {
        if self.pending.chooser.is_some() {
            self.toast(ToastKind::Info, "A file chooser is already open.");
            return;
        }
        let (purpose, kind, title) = if folder {
            (
                Chooser::QueueFolder,
                Choose::Folder,
                "Add a folder of videos",
            )
        } else {
            (Chooser::QueueVideos, Choose::Videos, "Add videos")
        };
        let answer = portal::choose(kind, title, self.env.wake.clone());
        self.pending.chooser = Some((purpose, answer));
    }

    fn chosen(&mut self, purpose: Chooser, paths: Vec<std::path::PathBuf>) {
        match purpose {
            Chooser::Setting(field) => {
                if let Some(path) = paths.into_iter().next() {
                    self.chosen_setting(field, path);
                }
            }
            Chooser::QueueVideos | Chooser::QueueFolder => {
                self.apply(vec![super::Action::QueueVideos(paths)]);
            }
        }
    }
}

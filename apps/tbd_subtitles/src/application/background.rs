//! The threads the window waits on, and folding their answers in before each frame.
//!
//! **Role:** hold the receiving end of every thread the application started (the desktop's
//! chooser, the files the desktop was asked to open, the model download, the machine checks, the
//! work folder's size, the desktop's colour scheme) and apply what they sent; let the toasts
//! whose time is up go.
//!
//! **Position:** owned by `TbdSubtitlesApp`; polled by `window` before each frame; the settings
//! part lives in `actions::settings`.
//!
//! **Signals and state:** channels only; each thread wakes the window after it sends.
//!
//! **Invariants:** at most one chooser is open at a time, and a chooser or an open that fails
//! says so in a red toast; a closed channel ends the wait.

use std::path::Path;
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use super::TbdSubtitlesApp;
use super::actions::{poll_runner, poll_settings};
use crate::core::color_scheme::{self, Scheme};
use crate::core::portal::{self, Choose, Chosen, Opened};
use crate::core::toast::ToastKind;
use crate::settings::events::PathField;
use crate::settings::models::machine::Check;
use crate::settings::services::model_downloads::Downloading;

/// How long the window waits for the desktop's colour scheme before its first frame; the portal
/// answers in a few milliseconds.
const FIRST_SCHEME: Duration = Duration::from_millis(250);

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
    pub(crate) scheme: Option<Receiver<Scheme>>,
    /// The desktop's answers to the files it was asked to open, each with the words a failure
    /// starts with.
    pub(crate) opens: Vec<(String, Receiver<Opened>)>,
}

impl TbdSubtitlesApp {
    /// Apply everything the threads sent since the last frame.
    pub(crate) fn poll(&mut self) {
        poll_settings(self);
        poll_runner(self);
        self.poll_chooser();
        self.poll_scheme();
        self.poll_opens();
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
        self.pending.opens.push((failed, answer));
    }

    /// Say in a red toast when the desktop could not open a file; forget the answered requests.
    fn poll_opens(&mut self) {
        let mut failures = Vec::new();
        self.pending
            .opens
            .retain(|(failed, answer)| match answer.try_recv() {
                Ok(Ok(())) | Err(TryRecvError::Disconnected) => false,
                Ok(Err(error)) => {
                    failures.push(format!("{failed}: {error}"));
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

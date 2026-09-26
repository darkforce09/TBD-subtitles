//! The threads the window waits on, and folding their answers in before each frame.
//!
//! **Role:** hold the receiving end of every thread the application started (the desktop's
//! chooser, the model download, the machine checks, the work folder's size) and apply what they
//! sent.
//!
//! **Position:** owned by `TbdSubtitlesApp`; polled by `window` before each frame; the settings
//! part lives in `actions::settings`.
//!
//! **Signals and state:** channels only; each thread wakes the window after it sends.
//!
//! **Invariants:** at most one chooser is open at a time; a closed channel ends the wait.

use std::sync::mpsc::{Receiver, TryRecvError};

use super::TbdSubtitlesApp;
use super::actions::{poll_runner, poll_settings};
use crate::core::portal::{self, Choose, Chosen};
use crate::settings::events::PathField;
use crate::settings::models::machine::Check;
use crate::settings::services::model_downloads::Downloading;

/// What a chooser's answer is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Chooser {
    Setting(PathField),
    QueueVideos,
    QueueFolder,
}

/// The threads the application waits on.
#[derive(Default)]
pub(crate) struct Pending {
    pub(crate) chooser: Option<(Chooser, Receiver<Chosen>)>,
    pub(crate) download: Option<Downloading>,
    pub(crate) checks: Option<Receiver<Vec<Check>>>,
    pub(crate) work_size: Option<Receiver<u64>>,
}

impl TbdSubtitlesApp {
    /// Apply everything the threads sent since the last frame.
    pub(crate) fn poll(&mut self) {
        poll_settings(self);
        poll_runner(self);
        self.poll_chooser();
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
            Err(error) => tracing::warn!(%error, "the desktop's chooser failed"),
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

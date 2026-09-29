//! Composition of the Check Text feature with the existing correction queue.
//!
//! **Role:** load text, persist corrections and refresh the actual subtitle preview.
//! **Position:** application actions above text review services.
//! **Signals and state:** background result channels, saves carrying their owning videos and one
//! bounded preview player. Pending saves outlive selection changes and a closed text view.
//! **Invariants:** model work and frame decoding never block the window; each saved correction
//! queues its own video's update exactly once, and never replaces another video's session.

use crate::application::TbdSubtitlesApp;
use crate::core::toast::ToastKind;
use crate::job_queue::models::queue::JobId;
use crate::job_queue::services::queue_editing;
use crate::line_review::{events::ReviewEvent, services::review_loading};
use crate::settings::services::job_settings;
use crate::text_review::models::{Event, Session};
use crate::text_review::services::{player, session};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Default)]
pub(crate) struct State {
    pub(crate) job: Option<JobId>,
    pub(crate) session: Option<Session>,
    pub(crate) pending: Option<Receiver<Result<Session, String>>>,
    pub(crate) saving: Option<PendingSave>,
    parked_saves: Vec<PendingSave>,
    pub(crate) player: Option<player::Player>,
    pub(crate) error: Option<String>,
    selected_text: Option<String>,
    preserved_draft: Option<(String, job_model::onscreen::TextEdit)>,
}

pub(crate) struct PendingSave {
    video: PathBuf,
    result: Receiver<Result<(), String>>,
}

impl State {
    fn take_saves(&mut self) -> Vec<PendingSave> {
        let mut saves = std::mem::take(&mut self.parked_saves);
        saves.extend(self.saving.take());
        saves
    }

    fn keep_save(&mut self, save: PendingSave, video: Option<&Path>) {
        if video == Some(save.video.as_path()) && self.saving.is_none() {
            self.saving = Some(save);
        } else {
            self.parked_saves.push(save);
        }
    }

    fn reset_view(&mut self, job: Option<JobId>, video: Option<&Path>) {
        let saves = self.take_saves();
        let preserved_draft = if self.job == job {
            self.session
                .as_ref()
                .and_then(|session| {
                    let text = session.document.occurrences.get(session.selected)?;
                    let draft = session.draft.as_ref()?;
                    let saved = session
                        .corrections
                        .edits
                        .get(&text.id)
                        .filter(|edit| edit.matches_source(text))
                        .cloned()
                        .unwrap_or_else(|| job_model::onscreen::TextEdit::from_occurrence(text));
                    (draft != &saved).then(|| (text.id.clone(), draft.clone()))
                })
                .or_else(|| self.preserved_draft.take())
        } else {
            None
        };
        let selected_text = if self.job == job {
            self.session
                .as_ref()
                .and_then(|session| session.document.occurrences.get(session.selected))
                .map(|text| text.id.clone())
                .or_else(|| self.selected_text.take())
        } else {
            None
        };
        *self = Self {
            job,
            selected_text,
            preserved_draft,
            ..Self::default()
        };
        for save in saves {
            self.keep_save(save, video);
        }
    }
}

impl TbdSubtitlesApp {
    pub(crate) fn open_text(&mut self, id: JobId) {
        let Some(video) = self.queue.get(id).map(|item| item.video.clone()) else {
            return;
        };
        self.apply_review(ReviewEvent::Close);
        self.text.reset_view(Some(id), Some(&video));
        let root = match job_settings::work_root(&self.settings.saved) {
            Ok(root) => root,
            Err(error) => {
                self.text.error = Some(error.to_string());
                return;
            }
        };
        let (send, receive) = mpsc::channel();
        self.text.pending = Some(receive);
        let wake = self.env.wake.clone();
        std::thread::spawn(move || {
            let result =
                review_loading::work_dir(&video, &root).and_then(|work| session::load(&work));
            let _ = send.send(result);
            wake();
        });
    }

    pub(crate) fn refresh_text(&mut self) {
        if let Some(id) = self.text.job {
            self.open_text(id);
        }
    }

    /// Closing the editor stops preview work while its correction writes still finish.
    pub(crate) fn close_text(&mut self) {
        self.text.reset_view(None, None);
    }

    pub(crate) fn poll_text(&mut self) {
        if let Some(result) = self
            .text
            .pending
            .as_ref()
            .and_then(|pending| pending.try_recv().ok())
        {
            self.text.pending = None;
            match result {
                Ok(mut session) => {
                    if let Some(index) = self.text.selected_text.as_ref().and_then(|id| {
                        session
                            .document
                            .occurrences
                            .iter()
                            .position(|text| &text.id == id)
                    }) {
                        select(&mut session, index);
                    }
                    self.text.selected_text = session
                        .document
                        .occurrences
                        .get(session.selected)
                        .map(|text| text.id.clone());
                    if let Some((id, draft)) = self.text.preserved_draft.take() {
                        let matches = session
                            .document
                            .occurrences
                            .get(session.selected)
                            .is_some_and(|text| text.id == id && draft.matches_source(text));
                        if matches {
                            session.draft = Some(draft);
                        } else {
                            session.error = Some(format!(
                                "The unsaved correction for {id} was not restored because its source text changed or is no longer available. Check the current Japanese and enter the correction again before saving."
                            ));
                        }
                    }
                    self.text.session = Some(session);
                    self.text_preview(false);
                }
                Err(error) => self.text.error = Some(error),
            }
        }
        let video = self
            .text
            .job
            .and_then(|id| self.queue.get(id))
            .map(|item| item.video.clone());
        let mut queued = false;
        for save in self.text.take_saves() {
            let result=match save.result.try_recv() {
                Ok(result)=>result,
                Err(TryRecvError::Empty)=>{self.text.keep_save(save,video.as_deref());continue;},
                Err(TryRecvError::Disconnected)=>Err("The correction save stopped before reporting its result. Reopen Check Text and save the change again.".into()),
            };
            match result {
                Ok(()) => {
                    queue_editing::queue_review(&mut self.queue, save.video, 1);
                    queued = true;
                }
                Err(error) => {
                    if let Some(session) = self
                        .text
                        .session
                        .as_mut()
                        .filter(|session| session.video == save.video)
                    {
                        session.error = Some(error.clone());
                    }
                    self.toast(
                        ToastKind::Error,
                        format!(
                            "On-screen correction for {} could not be saved: {error}",
                            save.video.display()
                        ),
                    );
                }
            }
        }
        if queued {
            self.save_queue();
            self.start_next();
        }
    }

    pub(crate) fn apply_text(&mut self, event: Event) {
        let Some(session) = self.text.session.as_mut() else {
            return;
        };
        let mut preview = false;
        match event {
            Event::Select(index) => {
                if select(session, index) {
                    preview = true;
                }
            }
            Event::Edit(edit) => session.draft = Some(edit),
            Event::Seek(time) => {
                session.position_s = time.clamp(0.0, session.duration_s);
                preview = true;
            }
            Event::FlaggedOnly(value) => session.flagged_only = value,
            Event::Stop => {
                if let Some(player) = &self.text.player {
                    player.stop();
                    if let Some(comparison) = player.comparison() {
                        session.position_s = comparison.time_s.clamp(0.0, session.duration_s);
                    }
                }
            }
            Event::Play => {
                self.text_preview(true);
                return;
            }
            Event::Save | Event::Undo | Event::Retry | Event::DiscardOrphans => {
                if self.text.saving.is_some()
                    || self
                        .text
                        .parked_saves
                        .iter()
                        .any(|save| save.video == session.video)
                {
                    return;
                }
                let saved_session = session.clone();
                if matches!(event, Event::Undo | Event::Retry | Event::DiscardOrphans) {
                    session.draft = None;
                    self.text.preserved_draft = None;
                }
                let session = saved_session;
                let (send, receive) = mpsc::channel();
                self.text.saving = Some(PendingSave {
                    video: session.video.clone(),
                    result: receive,
                });
                let wake = self.env.wake.clone();
                std::thread::spawn(move || {
                    let result = session::save(&session, &event);
                    let _ = send.send(result);
                    wake();
                });
            }
        }
        if preview {
            self.text_preview(false);
        }
    }

    fn text_preview(&mut self, play: bool) {
        if let Some(session) = &self.text.session {
            self.text.player = Some(player::start(
                session,
                session.position_s,
                play,
                self.env.wake.clone(),
            ));
        }
    }
}

/// Selecting or restoring an occurrence starts from its effective saved correction.
fn select(session: &mut Session, index: usize) -> bool {
    let Some(text) = session.document.occurrences.get(index) else {
        return false;
    };
    let edit = session
        .corrections
        .edits
        .get(&text.id)
        .filter(|edit| edit.matches_source(text))
        .cloned()
        .unwrap_or_else(|| job_model::onscreen::TextEdit::from_occurrence(text));
    session.selected = index;
    session.position_s = edit.start_s;
    session.draft = Some(edit);
    session.error = None;
    true
}

#[cfg(test)]
#[path = "tests/text.rs"]
mod tests;

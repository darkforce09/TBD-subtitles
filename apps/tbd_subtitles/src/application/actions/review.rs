//! The line review's actions: opening a finished job's review, editing a line, saving it and
//! queueing its review run, playing its clip, and reloading the lines after the run.
//!
//! **Role:** turn each `ReviewEvent` into a change of the review session, the corrections file,
//! the queue or the clip player.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply` and after a job ends; uses
//! `line_review::services` and the queue's editing.
//!
//! **Signals and state:** the review session and the clip player; `review.json` through
//! `review_editing`.
//!
//! **Invariants:** a saved correction always queues one review run of its job; a review run never
//! starts while a full run of the same video runs; a review that cannot open says why in a red
//! toast and leaves the report as it is; a review closes once its job is no longer finished.

use media_io::preview::Clip;

use crate::application::TbdSubtitlesApp;
use crate::core::toast::ToastKind;
use crate::job_queue::models::queue::{JobId, JobKind, JobState};
use crate::job_queue::services::queue_editing;
use crate::line_review::events::ReviewEvent;
use crate::line_review::services::clip_player::{self, ClipRequest};
use crate::line_review::services::{review_editing, review_loading};
use crate::settings::services::job_settings;

/// Seconds of sound played before and after a line.
const CLIP_PAD_S: f64 = 0.75;

impl TbdSubtitlesApp {
    /// Open the selected job's review, at `line` when one is named, else at its first flagged
    /// line.
    pub(crate) fn open_review(&mut self, line: Option<String>) {
        let Some(item) = self.queue.selected.and_then(|id| self.queue.get(id)) else {
            return;
        };
        let (id, video) = (item.id, item.video.clone());
        let loaded = job_settings::work_root(&self.settings.saved)
            .map_err(|e| format!("{e:#}"))
            .and_then(|root| review_loading::work_dir(&video, &root))
            .and_then(|work_dir| review_loading::load(&video, &work_dir));
        match loaded {
            Ok(mut session) => {
                let first = line.or_else(|| session.shown().next().map(|l| l.id.clone()));
                if let Some(first) = first {
                    if session.line(&first).is_some_and(|l| !l.flagged()) {
                        session.show_all = true;
                    }
                    review_editing::open(&mut session, &first);
                }
                self.review = Some((id, session));
            }
            Err(error) => {
                self.toast(
                    ToastKind::Error,
                    format!("Check Lines cannot open: {error}"),
                );
            }
        }
    }

    /// Close the line review once its job is no longer finished: tried or run again.
    pub(crate) fn close_review_unless_finished(&mut self) {
        let finished = |id: JobId| {
            self.queue.get(id).is_some_and(|item| {
                matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore)
            })
        };
        if self.review.as_ref().is_some_and(|(id, _)| !finished(*id)) {
            self.apply_review(ReviewEvent::Close);
        }
    }

    pub(crate) fn apply_review(&mut self, event: ReviewEvent) {
        if matches!(event, ReviewEvent::Close) {
            self.stop_clip();
            self.review = None;
            return;
        }
        let Some((job, session)) = &mut self.review else {
            return;
        };
        let job = *job;
        match event {
            ReviewEvent::Open(id) => {
                review_editing::open(session, &id);
                self.stop_clip();
            }
            ReviewEvent::Pick(tag) => review_editing::pick(session, &tag),
            ReviewEvent::EditText(text) => {
                if let Some(draft) = &mut session.draft {
                    draft.text = text;
                }
            }
            ReviewEvent::SetFlags(flags) => {
                if let Some(draft) = &mut session.draft {
                    draft.flags = flags;
                }
            }
            ReviewEvent::Save => match review_editing::save(session) {
                Ok(()) => self.queue_review_run(job),
                Err(error) => session.notice = Some(error),
            },
            ReviewEvent::Revert(id) => match review_editing::revert(session, &id) {
                Ok(()) => self.queue_review_run(job),
                Err(error) => session.notice = Some(error),
            },
            ReviewEvent::Step { forward } => {
                let next = session
                    .draft
                    .as_ref()
                    .and_then(|d| review_editing::neighbour(session, &d.id, forward));
                if let Some(next) = next {
                    review_editing::open(session, &next);
                    self.stop_clip();
                }
            }
            ReviewEvent::ShowAll(all) => session.show_all = all,
            ReviewEvent::Play(sound) => {
                let request = session
                    .draft
                    .as_ref()
                    .and_then(|d| session.line(&d.id))
                    .map(|line| ClipRequest {
                        video: session.video.clone(),
                        audio_position: session.audio_position,
                        vocals: session.work_dir.join("audio").join("vocals_16k.f32"),
                        picture: session.picture,
                        clip: Clip::around(line.start_s, line.end_s, CLIP_PAD_S),
                        sound,
                    });
                self.stop_clip();
                if let Some(request) = request {
                    self.clip = Some(clip_player::play(request, self.env.wake.clone()));
                }
            }
            ReviewEvent::Stop => self.stop_clip(),
            ReviewEvent::Close => {}
        }
    }

    fn stop_clip(&mut self) {
        if let Some(clip) = self.clip.take() {
            clip.stop();
        }
    }

    /// Queue a review run of job `job`'s video; it starts at once unless a full run of the same
    /// video is running.
    fn queue_review_run(&mut self, job: JobId) {
        let Some(video) = self.queue.get(job).map(|item| item.video.clone()) else {
            return;
        };
        queue_editing::queue_review(&mut self.queue, video);
        self.save_queue();
        self.start_next();
    }

    /// Whether a full run of job `job`'s video is running now.
    pub(crate) fn video_busy(&self, job: JobId) -> bool {
        self.video_running(job, JobKind::Full)
    }

    /// Read the open review's lines again after a run of its video ended, keeping the line open.
    pub(crate) fn refresh_review(&mut self) {
        let Some((job, session)) = &self.review else {
            return;
        };
        let running =
            self.queue.items.iter().any(|item| {
                item.video == session.video && matches!(item.state, JobState::Running(_))
            });
        if running {
            return;
        }
        let (job, open, show_all) = (
            *job,
            session.draft.as_ref().map(|d| d.id.clone()),
            session.show_all,
        );
        if let Ok(mut fresh) = review_loading::load(&session.video, &session.work_dir) {
            fresh.show_all = show_all;
            fresh.notice = session.notice.clone();
            if let Some(open) = open {
                review_editing::open(&mut fresh, &open);
            }
            self.review = Some((job, fresh));
        }
    }
}

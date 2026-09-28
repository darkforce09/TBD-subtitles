//! The line review's actions: opening a finished job's review (on a group, when the Overview
//! names one), editing a line, saving it or keeping it and queueing its review run, following
//! that run, playing the clip and decoding the open line's still frame, and reading the lines
//! again after a run.
//!
//! **Role:** turn each `ReviewEvent` into a change of the review session, the corrections file,
//! the queue, the clip player or the still frame.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply`, by the runner when a review run
//! starts or ends, and after a job ends; uses `line_review::services` and the queue's editing.
//!
//! **Signals and state:** the review session, the clip player, the open line's still frame and
//! the parked edits and runs of closed reviews; `review.json` through `review_editing`.
//!
//! **Invariants:** a saved correction always queues one review run of its job; a review run never
//! starts while a full run of the same video runs; a review that cannot open, or a line that
//! cannot be saved, says why in a red toast; a review closes once its job is no longer finished,
//! and its unsaved edits and runs wait in `parked` until it opens again, following the runs of its
//! video meanwhile; taking back a line with no correction queues nothing; the clip stops and the
//! new line's still frame, at its start, is decoded whenever the editor shows another line.

use std::path::Path;

use media_io::preview::Clip;

use crate::application::TbdSubtitlesApp;
use crate::core::toast::ToastKind;
use crate::job_queue::models::queue::{JobId, JobKind, JobState};
use crate::job_queue::services::queue_editing;
use crate::job_report::models::finding_group::LineGroup;
use crate::line_review::events::ReviewEvent;
use crate::line_review::services::clip_player::{self, ClipRequest, PAD_S};
use crate::line_review::services::{line_filter, review_editing, review_loading};
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    /// Open the selected job's review on its lines to check, narrowed to `group` when one is
    /// named, with the edits it had when it last closed.
    pub(crate) fn open_review(&mut self, group: Option<LineGroup>) {
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
                self.close_review();
                if let Some(parked) = self.parked.remove(&id) {
                    review_editing::unpark(&mut session, parked);
                }
                session.group = group;
                self.review = Some((id, session));
                self.follow_open_line();
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

    /// Close the review, keeping its unsaved edits and its runs for when it opens again.
    fn close_review(&mut self) {
        self.stop_clip();
        self.still = None;
        if let Some((job, session)) = self.review.take() {
            let parked = review_editing::park(session);
            if parked.drafts.is_empty() && parked.runs.is_empty() {
                self.parked.remove(&job);
            } else {
                self.parked.insert(job, parked);
            }
        }
    }

    pub(crate) fn apply_review(&mut self, event: ReviewEvent) {
        if matches!(event, ReviewEvent::Close) {
            self.close_review();
            return;
        }
        let Some((job, session)) = &mut self.review else {
            return;
        };
        let job = *job;
        let saved = match event {
            ReviewEvent::Open(id) => {
                review_editing::open(session, &id);
                None
            }
            ReviewEvent::Pick(tag) => {
                review_editing::pick(session, &tag);
                None
            }
            ReviewEvent::EditText(text) => {
                review_editing::edit_text(session, text);
                None
            }
            ReviewEvent::SetFlags(flags) => {
                review_editing::set_flags(session, flags);
                None
            }
            ReviewEvent::Discard => {
                review_editing::discard(session);
                None
            }
            ReviewEvent::Save => Some(review_editing::save(session).map(drop)),
            ReviewEvent::LooksRight => Some(review_editing::looks_right(session).map(drop)),
            // A line with no correction has nothing to take back, and no run to queue.
            ReviewEvent::Revert(id) => match review_editing::revert(session, &id) {
                Ok(false) => None,
                taken => Some(taken.map(drop)),
            },
            ReviewEvent::Step { forward } => {
                if let Some(next) = line_filter::neighbour(session, forward) {
                    review_editing::open(session, &next);
                }
                None
            }
            ReviewEvent::List(list) => {
                session.list = list;
                None
            }
            ReviewEvent::Search(text) => {
                session.search = text;
                None
            }
            ReviewEvent::ClearGroup => {
                session.group = None;
                None
            }
            ReviewEvent::Play(sound) => {
                let request = line_filter::open_line(session).map(|line| ClipRequest {
                    video: session.video.clone(),
                    audio_position: session.audio_position,
                    vocals: session.work_dir.join("audio").join("vocals_16k.f32"),
                    picture: session.picture,
                    clip: Clip::around(line.start_s, line.end_s, PAD_S),
                    sound,
                });
                self.stop_clip();
                if let Some(request) = request {
                    self.clip = Some(clip_player::play(request, self.env.wake.clone()));
                }
                None
            }
            ReviewEvent::Stop => {
                self.stop_clip();
                None
            }
            ReviewEvent::Close => None,
        };
        match saved {
            Some(Ok(())) => self.queue_review_run(job),
            Some(Err(error)) => self.toast(ToastKind::Error, error),
            None => {}
        }
        self.follow_open_line();
    }

    /// Keep the review on the line its editor shows: when that line changed, stop the clip and
    /// decode the new line's still frame, if its video has a picture.
    fn follow_open_line(&mut self) {
        let Some((_, session)) = &mut self.review else {
            self.still = None;
            return;
        };
        let open = line_filter::open_line(session).map(|line| (line.id.clone(), line.start_s));
        session.open = open.as_ref().map(|(id, _)| id.clone());
        if self.still.as_ref().map(|(id, _)| id) == open.as_ref().map(|(id, _)| id) {
            return;
        }
        let (video, picture) = (session.video.clone(), session.picture);
        self.stop_clip();
        // The frame at the line's start, where the playhead rests until Play.
        self.still = open.map(|(id, start_s)| {
            let still = (picture != (0, 0) && video.is_file())
                .then(|| clip_player::still(video, picture, start_s, self.env.wake.clone()));
            (id, still)
        });
    }

    fn stop_clip(&mut self) {
        if let Some(clip) = self.clip.take() {
            clip.stop();
        }
    }

    /// Queue a review run of job `job`'s video; it starts at once unless a full run of the same
    /// video is running. The corrections file changed, so the video's rows count their lines to
    /// check again and the Overview reads its report again.
    fn queue_review_run(&mut self, job: JobId) {
        let Some(video) = self.queue.get(job).map(|item| item.video.clone()) else {
            return;
        };
        self.refresh_summaries(Some(&video));
        self.refresh_report(true);
        queue_editing::queue_review(&mut self.queue, video);
        self.save_queue();
        self.start_next();
    }

    /// Whether a full run of job `job`'s video is running now.
    pub(crate) fn video_busy(&self, job: JobId) -> bool {
        self.video_running(job, JobKind::Full)
    }

    /// A review run of `video` started: the saved lines of that video, in the open review or a
    /// closed one, are being updated.
    pub(crate) fn review_run_started(&mut self, video: &Path) {
        if let Some((_, session)) = &mut self.review
            && session.video == video
        {
            review_editing::run_started(session);
        }
        for (job, parked) in &mut self.parked {
            if self.queue.get(*job).is_some_and(|item| item.video == video) {
                review_editing::start_runs(&mut parked.runs);
            }
        }
    }

    /// A review run of `video` ended, well when `ok`: its lines are updated, or failed.
    pub(crate) fn review_run_ended(&mut self, video: &Path, ok: bool) {
        if let Some((_, session)) = &mut self.review
            && session.video == video
        {
            review_editing::run_ended(session, ok);
        }
        for (job, parked) in &mut self.parked {
            if self.queue.get(*job).is_some_and(|item| item.video == video) {
                review_editing::end_runs(&mut parked.runs, ok);
            }
        }
    }

    /// Read the open review's lines again after a run of its video ended, keeping what the owner
    /// did: the open line, the list, the search, the group, the runs and the edits.
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
        let job = *job;
        if let Ok(mut fresh) = review_loading::load(&session.video, &session.work_dir) {
            review_editing::carry_over(session, &mut fresh);
            self.review = Some((job, fresh));
            self.follow_open_line();
        }
    }
}

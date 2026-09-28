//! The finished jobs' reports: the selected job's report read from its work directory, each
//! finished row's summary, and the Overview's requests.
//!
//! **Role:** read the selected finished job's report when it is selected or ends; read the
//! summary of every finished row when the window opens, and of a video's rows after each of its
//! runs and corrections; turn each `ReportEvent` into a desktop request, a toast, Check Lines or a
//! run again from the language-model calls.
//!
//! **Position:** called by `application::TbdSubtitlesApp::apply`, `new`, the runner's poll and
//! the review's save; uses `job_report::services`.
//!
//! **Signals and state:** the selected job's report and the row summaries.
//!
//! **Invariants:** a row's summary is the one its files give now, or none when they cannot be
//! read; Try Again from the Overview reruns the language-model calls and every step after them.

use std::path::Path;

use job_model::StepName;

use crate::application::TbdSubtitlesApp;
use crate::application::background::Opening;
use crate::core::toast::ToastKind;
use crate::job_queue::events::JobQueueEvent;
use crate::job_queue::models::queue::JobState;
use crate::job_report::events::{LinesToCheck, ReportEvent};
use crate::job_report::services::{line_counts, report_loading};
use crate::line_review::events::ReviewEvent;
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    pub(crate) fn apply_report(&mut self, event: ReportEvent) {
        match event {
            ReportEvent::OpenVideo(path) => self.open_with_desktop(Opening::Play, &path),
            ReportEvent::ShowInFolder(path) => self.open_with_desktop(Opening::Reveal, &path),
            ReportEvent::OpenReport(path) => self.open_with_desktop(Opening::Read, &path),
            ReportEvent::Copied => self.toast(ToastKind::Success, "Subtitle path copied"),
            ReportEvent::CheckLines(lines) => self.check_lines(lines),
            ReportEvent::TryAgain => {
                if let Some(id) = self.queue.selected {
                    self.apply_queue(JobQueueEvent::TryAgain(id, Some(StepName::Adjudicate)));
                }
            }
        }
    }

    /// Open Check Lines on `lines`: a group opens at its earliest line, since the line review
    /// lists every line worth a listen and no group alone; a time shows every line, at the one
    /// nearest it.
    fn check_lines(&mut self, lines: LinesToCheck) {
        let first = match lines {
            LinesToCheck::Group(group) => self
                .report
                .as_ref()
                .and_then(|(_, report)| report.as_ref().ok())
                .and_then(|report| line_counts::first_line(&report.qc, group)),
            LinesToCheck::Flagged | LinesToCheck::Near(_) => None,
        };
        self.open_review(first);
        let LinesToCheck::Near(at) = lines else {
            return;
        };
        let nearest = self.review.as_ref().and_then(|(_, session)| {
            let distance = |start: f64, end: f64| {
                if at < start {
                    start - at
                } else {
                    (at - end).max(0.0)
                }
            };
            session
                .lines
                .iter()
                .min_by(|a, b| {
                    distance(a.start_s, a.end_s).total_cmp(&distance(b.start_s, b.end_s))
                })
                .map(|line| line.id.clone())
        });
        self.apply_review(ReviewEvent::ShowAll(true));
        if let Some(id) = nearest {
            self.apply_review(ReviewEvent::Open(id));
        }
    }

    /// Read the selected job's report when it is finished and not read yet; forget it otherwise.
    pub(crate) fn refresh_report(&mut self, force: bool) {
        let Some(item) = self.queue.selected.and_then(|id| self.queue.get(id)) else {
            self.report = None;
            return;
        };
        if !matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore) {
            self.report = None;
            return;
        }
        if !force && self.report.as_ref().is_some_and(|(id, _)| *id == item.id) {
            return;
        }
        let loaded = job_settings::work_root(&self.settings.saved)
            .map_err(|e| format!("{e:#}"))
            .and_then(|root| report_loading::load(&item.video, &root));
        if let Ok(report) = &loaded {
            self.summaries.insert(item.id, report.summary());
        }
        self.report = Some((item.id, loaded));
    }

    /// Read the summary of every finished job of `video`, or of every finished job when none is
    /// named; a job whose files cannot be read has none.
    pub(crate) fn refresh_summaries(&mut self, video: Option<&Path>) {
        let Ok(root) = job_settings::work_root(&self.settings.saved) else {
            return;
        };
        for item in &self.queue.items {
            let finished = matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore);
            if !finished || video.is_some_and(|video| video != item.video) {
                continue;
            }
            match report_loading::summary(&item.video, &root) {
                Ok(summary) => {
                    self.summaries.insert(item.id, summary);
                }
                Err(_) => {
                    self.summaries.remove(&item.id);
                }
            }
        }
    }
}

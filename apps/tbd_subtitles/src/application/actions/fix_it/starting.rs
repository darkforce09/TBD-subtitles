//! Starting and stopping Fix It: on one video, on every finished video with lines to fix (Fix
//! All), or on a video whose full run just finished, when the owner asked for Fix It after each
//! job.
//!
//! **Role:** start a Fix It run with the model and processes saved now and a seat at the one
//! gate every run's `claude` calls share, or say why it cannot start; list the finished videos
//! Fix All would start on; stop the run of one video.
//!
//! **Position:** part of the application's Fix It actions (`super`); called by `Action::FixIt`,
//! `Action::StopFix` and the report's actions, the queue's `JobQueueEvent::FixAll`, and the
//! runner's poll once a full run finished; `feature_views` counts `fix_candidates` for the
//! sidebar's Fix All.
//!
//! **Signals and state:** adds runs to `Pending::fixes`; reads the saved settings, the queue, the
//! row summaries and the gate.
//!
//! **Invariants:** a run starts only where `fix_refusal` has nothing against it, so never twice
//! on one video; the candidates are the finished rows of Done with findings Fix It would ask
//! about and nothing against a run, one per video, oldest finished first; Fix It after each job starts only on a
//! full run that finished well, only while its setting is on, and says nothing when it starts.

use std::collections::HashSet;

use crate::application::TbdSubtitlesApp;
use crate::core::format::plural;
use crate::core::toast::ToastKind;
use crate::job_queue::models::queue::{JobId, JobState};
use crate::job_queue::models::sidebar::Section;
use crate::job_queue::services::sidebar_rows;
use crate::job_report::services::fix_it;
use crate::settings::models::claude_models;
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
    /// Start Fix It on job `id`'s video with the settings saved now, or say why it cannot start.
    fn begin_fix(&mut self, id: JobId) -> Result<(), String> {
        if let Some(reason) = self.fix_refusal(id) {
            return Err(reason.to_string());
        }
        let Some(item) = self.queue.get(id) else {
            return Err("The video is not in the list.".to_string());
        };
        let video = item.video.clone();
        let saved = &self.settings.saved;
        let work_root = job_settings::work_root(saved)
            .map_err(|error| format!("Fix It cannot start: {error:#}"))?;
        let options = pipeline::fix_it::FixOptions {
            work_root,
            model: saved.language_model.fix_model.clone(),
            glossary_name: job_settings::glossary_name(saved),
            processes: saved.language_model.processes.max(1),
            calls: self.claude_gate.seat(),
            cancel: pipeline::CancelToken::new(),
        };
        let model = claude_models::display_name(&options.model);
        let fixing = fix_it::start(
            self.env.fix_video.clone(),
            video,
            options,
            model,
            self.env.wake.clone(),
        );
        self.pending.fixes.insert(id, fixing);
        Ok(())
    }

    /// Start Fix It on job `id`'s video, or say in a red toast why it cannot start.
    pub(crate) fn start_fix(&mut self, id: JobId) {
        if let Err(reason) = self.begin_fix(id) {
            self.toast(ToastKind::Error, reason);
        }
    }

    /// Stop the Fix It run of job `id`'s video.
    pub(crate) fn stop_fix(&mut self, id: JobId) {
        let Some(video) = self.queue.get(id).map(|item| item.video.clone()) else {
            return;
        };
        for fixing in self.pending.fixes.values_mut() {
            if fixing.video == video {
                fixing.stop();
            }
        }
    }

    /// The finished jobs Fix All starts on: the rows of Done whose job finished, with findings
    /// Fix It would ask about and nothing against a run, the newest row of each video only,
    /// oldest finished first (Done lists the newest first), so a batch goes roughly in episode
    /// order.
    pub(crate) fn fix_candidates(&self) -> Vec<JobId> {
        let mut videos = HashSet::new();
        let mut candidates: Vec<JobId> = sidebar_rows::rows(&self.queue)
            .into_iter()
            .filter(|row| row.section == Section::Done)
            .filter(|row| {
                self.queue.get(row.id).is_some_and(|item| {
                    matches!(item.state, JobState::Finished(_) | JobState::FinishedBefore)
                })
            })
            .filter(|row| self.summaries.get(&row.id).is_some_and(|s| s.fixable > 0))
            .filter(|row| self.fix_refusal(row.id).is_none())
            .filter(|row| videos.insert(row.video.clone()))
            .map(|row| row.id)
            .collect();
        candidates.reverse();
        candidates
    }

    /// Start Fix It on every candidate, and say on how many; a run that cannot start says why in
    /// a red toast, the first only.
    pub(crate) fn fix_all(&mut self) {
        let (mut started, mut refused) = (0, None);
        for id in self.fix_candidates() {
            match self.begin_fix(id) {
                Ok(()) => started += 1,
                Err(reason) => {
                    let name = self.queue.get(id).map(|item| item.short_name());
                    let name = name.unwrap_or_default();
                    refused.get_or_insert(format!("Fix It could not start on {name}: {reason}"));
                }
            }
        }
        if started > 0 {
            let videos = plural(started, "video");
            self.toast(ToastKind::Info, format!("Fix It started on {videos}."));
        }
        match refused {
            Some(reason) => self.toast(ToastKind::Error, reason),
            None if started == 0 => {
                self.toast(ToastKind::Info, "No finished video has lines to fix.");
            }
            None => {}
        }
    }

    /// With Fix It after each job on, start Fix It on each job of `ended`, full runs that just
    /// finished well, that is a candidate; a run that cannot start says why in a red toast.
    pub(crate) fn fix_after_run(&mut self, ended: &[JobId]) {
        if !self.settings.saved.language_model.fix_after_run || ended.is_empty() {
            return;
        }
        let candidates = self.fix_candidates();
        for id in ended.iter().filter(|id| candidates.contains(id)) {
            if let Err(reason) = self.begin_fix(*id) {
                let name = self.queue.get(*id).map(|item| item.short_name());
                let name = name.unwrap_or_default();
                let text = format!("Fix It could not start on {name}: {reason}");
                self.toast(ToastKind::Error, text);
            }
        }
    }
}

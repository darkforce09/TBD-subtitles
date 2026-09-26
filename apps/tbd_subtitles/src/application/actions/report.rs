//! The selected job's report: read from its work directory when a finished job is selected or
//! the selected job ends.

use crate::application::TbdSubtitlesApp;
use crate::job_queue::models::queue::JobState;
use crate::job_report::services::report_loading;
use crate::settings::services::job_settings;

impl TbdSubtitlesApp {
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
        self.report = Some((item.id, loaded));
    }
}

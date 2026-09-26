//! A finished job's report as the window shows it: the quality check, the files, and each step's
//! time and memory.

use std::path::PathBuf;

use job_model::StepName;
use job_model::job::StepMeasure;
use job_model::report::QcReport;

/// Everything the report view shows about one job.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JobReport {
    pub(crate) video: PathBuf,
    pub(crate) work_dir: PathBuf,
    /// The subtitle file beside the video.
    pub(crate) subtitles: PathBuf,
    /// `report.md` in the work directory.
    pub(crate) report_file: PathBuf,
    pub(crate) qc: QcReport,
    /// Each finished step with its measure, in run order.
    pub(crate) steps: Vec<(StepName, StepMeasure)>,
}

impl JobReport {
    /// The steps' wall time added up, the shot scan left out since it runs alongside.
    pub(crate) fn total_s(&self) -> f64 {
        self.steps
            .iter()
            .filter(|(step, _)| *step != StepName::ShotScan)
            .map(|(_, measure)| measure.wall_s)
            .sum()
    }
}

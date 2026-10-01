//! A finished job's report as the window shows it: the quality check with its problems and its
//! lines worth a listen, the owner's corrections, what Fix It did, the files (the localized video
//! among them), and each step's time and memory.

use std::path::PathBuf;

use job_model::StepName;
use job_model::job::StepMeasure;
use job_model::outputs::Corrections;
use job_model::report::QcReport;

use crate::job_report::models::fix_result::FixResult;
use crate::job_report::models::problem::Problem;
use crate::job_report::models::summary::{LineCounts, RowSummary};

/// What a job that replaces writing in a localized video left.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct LocalizedOutput {
    /// The occurrences drawn into the video, once the replacement steps have run.
    pub(crate) replaced: Option<usize>,
    /// The localized video beside the source, while the file is there.
    pub(crate) video: Option<PathBuf>,
    /// The localized video's subtitle file, while the file is there.
    pub(crate) subtitles: Option<PathBuf>,
}

/// Everything the Overview shows about one job.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct JobReport {
    pub(crate) visual: Option<job_model::onscreen::TextSummary>,
    /// What the localized video left, when the job writes one.
    pub(crate) localized: Option<LocalizedOutput>,
    pub(crate) video: PathBuf,
    pub(crate) work_dir: PathBuf,
    /// The subtitle file beside the video.
    pub(crate) subtitles: PathBuf,
    /// `report.md` in the work directory.
    pub(crate) report_file: PathBuf,
    pub(crate) qc: QcReport,
    /// The corrections, from the job database: the owner's and Fix It's; none before the first.
    pub(crate) corrections: Corrections,
    /// Why it does not pass the quality check; empty when it passes.
    pub(crate) problems: Vec<Problem>,
    /// Its lines worth a listen, and how many the owner and Claude checked.
    pub(crate) lines: LineCounts,
    /// How many of its findings Fix It would ask about; none hides the Fix It button.
    pub(crate) fixable: usize,
    /// What Fix It did, while a run of the job as it stands answered at least one line.
    pub(crate) fix_result: Option<FixResult>,
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

    /// What its sidebar row and the header's Check Lines say.
    pub(crate) fn summary(&self) -> RowSummary {
        RowSummary {
            problems: self.problems.len(),
            flagged: self.lines.flagged,
            to_check: self.lines.to_check(),
            fixed_by_claude: self.fix_result.is_some(),
            fixable: self.fixable,
        }
    }
}

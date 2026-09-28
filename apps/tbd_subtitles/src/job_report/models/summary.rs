//! A finished job summed up: its lines worth a listen in their groups, and the few numbers its
//! sidebar row and header show.

use crate::job_report::models::finding_group::LineGroup;

/// The lines worth a listen of one job.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LineCounts {
    /// The distinct lines with a finding in some group.
    pub(crate) flagged: usize,
    /// Of those, the lines the owner corrected (in `review.json`).
    pub(crate) checked: usize,
    /// Of those, the lines Claude settled and the owner did not: its changes the owner has not
    /// kept or undone, and the lines whose every finding Fix It answered.
    pub(crate) by_claude: usize,
    /// The distinct lines of each group that has any, in the groups' order.
    pub(crate) groups: Vec<(LineGroup, usize)>,
}

impl LineCounts {
    /// The flagged lines neither the owner nor Claude has checked yet.
    pub(crate) fn to_check(&self) -> usize {
        self.flagged
            .saturating_sub(self.checked)
            .saturating_sub(self.by_claude)
    }
}

/// What a finished job's sidebar row and the header's Check Lines say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RowSummary {
    /// The pass rules it breaks; none when it passes the quality check.
    pub(crate) problems: usize,
    /// The distinct lines worth a listen.
    pub(crate) flagged: usize,
    /// Of those, the lines not checked yet.
    pub(crate) to_check: usize,
    /// Whether a Fix It run of the job as it stands answered at least one line.
    pub(crate) fixed_by_claude: bool,
}

impl RowSummary {
    /// Whether the job passes the quality check.
    pub(crate) fn passes(&self) -> bool {
        self.problems == 0
    }
}

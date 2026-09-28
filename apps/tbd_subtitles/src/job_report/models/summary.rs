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
    /// The distinct lines of each group that has any, in the groups' order.
    pub(crate) groups: Vec<(LineGroup, usize)>,
}

impl LineCounts {
    /// The flagged lines the owner has not corrected yet.
    pub(crate) fn to_check(&self) -> usize {
        self.flagged.saturating_sub(self.checked)
    }
}

/// What a finished job's sidebar row and the header's Check Lines say.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct RowSummary {
    /// The pass rules it breaks; none when it passes the quality check.
    pub(crate) problems: usize,
    /// The distinct lines worth a listen.
    pub(crate) flagged: usize,
    /// Of those, the lines not corrected yet.
    pub(crate) to_check: usize,
}

impl RowSummary {
    /// Whether the job passes the quality check.
    pub(crate) fn passes(&self) -> bool {
        self.problems == 0
    }
}

//! The sidebar's rows: one per video, grouped into the sections Now, Up Next and Done.

use std::path::PathBuf;

use crate::job_queue::models::queue::JobId;

/// A group of rows in the sidebar, in the order they show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum Section {
    /// The running jobs.
    Now,
    /// The waiting jobs, in the order they run.
    UpNext,
    /// Finished, failed and cancelled jobs, the newest first.
    Done,
}

impl Section {
    pub(crate) const ALL: [Section; 3] = [Section::Now, Section::UpNext, Section::Done];

    /// The section's heading.
    pub(crate) fn title(self) -> &'static str {
        match self {
            Section::Now => "Now",
            Section::UpNext => "Up Next",
            Section::Done => "Done",
        }
    }
}

/// The correction runs waiting or running for a finished video, shown on its row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReviewFold {
    /// The corrections they carry.
    pub(crate) corrections: usize,
    /// The one running now, which Stop Updating Subtitles cancels.
    pub(crate) running: Option<JobId>,
}

/// One row of the sidebar.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SidebarRow {
    /// The row's job: a full run, or a correction run with no full run of its video to fold into.
    pub(crate) id: JobId,
    /// The video's name, with the corrections of a correction run shown on its own row.
    pub(crate) name: String,
    pub(crate) video: PathBuf,
    pub(crate) section: Section,
    /// The correction runs folded into this row, which leave the list with it.
    pub(crate) folded: Vec<JobId>,
    /// The folded correction runs still to finish.
    pub(crate) fold: Option<ReviewFold>,
    /// A waiting full run's place in line, from 1.
    pub(crate) place: Option<usize>,
    /// Whether the row can leave the list: nothing on it runs.
    pub(crate) removable: bool,
    /// Whether the row can be dragged to another place in line: a waiting full run.
    pub(crate) draggable: bool,
}

//! The Overview of a finished job: its file card, its localized video card, its lines card and its
//! two disclosures.

mod file_card;
mod fix_result_card;
mod lines_card;
mod localized_card;
mod overview;
mod report_details;

pub(crate) use overview::{OverviewView, overview_ui};

//! Applying actions after the frame, one module per feature.

mod fix_it;
mod queue;
mod report;
mod review;
mod runner;
mod settings;

pub(super) use fix_it::poll_fix;
pub(super) use runner::poll_runner;
pub(super) use settings::{new_settings_page, poll_settings};

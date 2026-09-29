//! Applying actions after the frame, one module per feature.

mod automation;
mod fix_it;
mod log_console;
mod queue;
mod report;
mod review;
mod runner;
mod settings;

#[cfg(test)]
pub(super) use automation::right_click_entry;
pub(super) use automation::{install_right_click, seeded_history};
pub(super) use fix_it::{FixFollowup, poll_fix};
pub(super) use log_console::poll_log;
pub(super) use runner::poll_runner;
pub(super) use settings::{new_settings_page, poll_settings};

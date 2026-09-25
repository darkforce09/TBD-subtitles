//! The `tbd-subtitles` binary.
//!
//! **Role:** the composition root: parses the command line and starts the desktop window, a
//! headless `process` run, or one GPU `worker` stage.
//!
//! **Position:** the top layer; `cli` parses and dispatches, `application` runs the eframe window,
//! `core` holds what every module shares, and each feature folder (`job_queue`, `job_report`,
//! `line_review`, `settings`) owns one part of the window.
//!
//! **Signals and state:** reads the command line and `RUST_LOG`; logs to stderr; exits 0 on
//! success and 1 with the error chain on stderr otherwise.
//!
//! **Invariants:** only this binary reports errors with `anyhow`; a command that cannot run says
//! so and exits non-zero, never reporting success.

mod application;
mod cli;
mod core;
mod job_queue;
mod job_report;
mod line_review;
mod settings;

use std::process::ExitCode;

fn main() -> ExitCode {
    core::logging::initialise();
    match cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("tbd-subtitles: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[path = "tests/architecture_rules.rs"]
mod architecture_rules;

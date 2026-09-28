//! A running job's events as log lines, for the log window and the log file.
//!
//! **Role:** turn each pipeline [`Progress`] event and a job's outcome into a level and a line of
//! text, and log them under the `job` target.
//!
//! **Position:** used by `job_runner` for every event it forwards; pure apart from [`emit`].
//!
//! **Signals and state:** [`ProgressLog`] remembers, per step, the last tenth of its progress it
//! logged.
//!
//! **Invariants:** a step's advance is logged once per tenth, never per chunk; a failure is an
//! error line; every other event is one info line.

use std::collections::BTreeMap;

use job_model::StepName;
use pipeline::progress::Progress;
use pipeline::{JobOutcome, PipelineError};
use tracing::Level;

/// The tenths of each step's progress already logged.
#[derive(Debug, Default)]
pub(crate) struct ProgressLog {
    tenths: BTreeMap<StepName, usize>,
}

impl ProgressLog {
    /// The line for `event`, or `None` when it says nothing new.
    pub(crate) fn describe(&mut self, event: &Progress) -> Option<(Level, String)> {
        let line = match event {
            Progress::JobStarted {
                video,
                work_dir,
                stale,
            } => {
                self.tenths.clear();
                let steps = if stale.is_empty() {
                    "nothing to redo".to_string()
                } else {
                    let names: Vec<String> = stale.iter().map(ToString::to_string).collect();
                    format!("to run: {}", names.join(", "))
                };
                format!(
                    "started {} in {}; {steps}",
                    video.display(),
                    work_dir.display()
                )
            }
            Progress::JobDuration(secs) => format!("the video runs {}", minutes(*secs)),
            Progress::StepSkipped(step) => format!("{step}: kept from an earlier run"),
            Progress::StepStarted(step) => {
                self.tenths.remove(step);
                format!("{step}: started")
            }
            Progress::StepAdvanced { step, done, total } => {
                let tenth = done.saturating_mul(10).checked_div(*total)?;
                if self.tenths.get(step).is_some_and(|&last| last >= tenth) {
                    return None;
                }
                self.tenths.insert(*step, tenth);
                return Some((Level::DEBUG, format!("{step}: {done} of {total}")));
            }
            Progress::StepMessage { step, text } => format!("{step}: {text}"),
            Progress::StepFinished { step, measure } => {
                let mut line = format!("{step}: finished in {:.1} s", measure.wall_s);
                if let Some(mib) = measure.peak_ram_mib {
                    line.push_str(&format!(", {mib:.0} MiB RAM"));
                }
                if let Some(mib) = measure.peak_vram_mib {
                    line.push_str(&format!(", {mib:.0} MiB VRAM"));
                }
                for (key, value) in &measure.notes {
                    line.push_str(&format!(", {key}={value}"));
                }
                line
            }
            Progress::StepFailed { step, message } => {
                return Some((Level::ERROR, format!("{step}: failed: {message}")));
            }
        };
        Some((Level::INFO, line))
    }
}

/// The line for a job's end.
pub(crate) fn describe_end(outcome: &Result<JobOutcome, PipelineError>) -> (Level, String) {
    match outcome {
        Ok(done) => (
            Level::INFO,
            format!(
                "finished: {} steps ran, {} kept; subtitles {}",
                done.ran.len(),
                done.skipped.len(),
                done.subtitles.display()
            ),
        ),
        Err(error) => (Level::ERROR, format!("stopped: {error}")),
    }
}

/// Log `text` under the `job` target, prefixed with the video's `name`.
pub(crate) fn emit(name: &str, (level, text): (Level, String)) {
    match level {
        Level::ERROR => tracing::error!(target: "job", "{name}: {text}"),
        Level::WARN => tracing::warn!(target: "job", "{name}: {text}"),
        Level::INFO => tracing::info!(target: "job", "{name}: {text}"),
        Level::DEBUG => tracing::debug!(target: "job", "{name}: {text}"),
        Level::TRACE => tracing::trace!(target: "job", "{name}: {text}"),
    }
}

fn minutes(secs: f64) -> String {
    format!("{:.1} min", secs / 60.0)
}

#[cfg(test)]
#[path = "tests/progress_log.rs"]
mod tests;

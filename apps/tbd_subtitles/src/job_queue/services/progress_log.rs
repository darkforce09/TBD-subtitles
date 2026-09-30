//! A running job's events as log lines, for the log window and the log file.
//!
//! **Role:** turn each pipeline [`Progress`] event and a job's outcome into a [`JobLine`]: a
//! level, the step it is about and its words; log them under the `job` target with the video and
//! step as fields; pass a worker's model call on as the exchange event the log window keeps.
//!
//! **Position:** used by `job_runner` for every event it forwards; pure apart from [`emit`] and
//! [`emit_call`].
//!
//! **Signals and state:** [`ProgressLog`] remembers, per step, the last tenth of its progress it
//! logged.
//!
//! **Invariants:** a step's advance is logged once per tenth, never per chunk; a failure is an
//! error line; the words never repeat the video or the step, which the window shows above them;
//! a model call is never a line.

use std::collections::BTreeMap;
use std::path::Path;

use inference::llm::call_log::EXCHANGE_TARGET;
use job_model::StepName;
use job_model::model_call::ModelExchange;
use pipeline::progress::Progress;
use pipeline::{JobOutcome, PipelineError};
use tracing::Level;

/// One line about a job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct JobLine {
    pub(crate) level: Level,
    /// The step it is about; `None` for the whole job.
    pub(crate) step: Option<StepName>,
    pub(crate) text: String,
}

impl JobLine {
    fn new(level: Level, step: Option<StepName>, text: impl Into<String>) -> JobLine {
        JobLine {
            level,
            step,
            text: text.into(),
        }
    }
}

/// The tenths of each step's progress already logged.
#[derive(Debug, Default)]
pub(crate) struct ProgressLog {
    tenths: BTreeMap<StepName, usize>,
}

impl ProgressLog {
    /// The line for `event`, or `None` when it says nothing new or is a model call.
    pub(crate) fn describe(&mut self, event: &Progress) -> Option<JobLine> {
        let info = |step, text: String| Some(JobLine::new(Level::INFO, step, text));
        match event {
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
                info(
                    None,
                    format!(
                        "Job started for {} in {}; {steps}",
                        video.display(),
                        work_dir.display()
                    ),
                )
            }
            Progress::JobDuration(secs) => info(None, format!("The video runs {}", minutes(*secs))),
            Progress::StepSkipped(step) => info(Some(*step), "Kept from an earlier run".into()),
            Progress::StepStarted(step) => {
                self.tenths.remove(step);
                info(Some(*step), "Step started".into())
            }
            Progress::StepAdvanced { step, done, total } => {
                let tenth = done.saturating_mul(10).checked_div(*total)?;
                if self.tenths.get(step).is_some_and(|&last| last >= tenth) {
                    return None;
                }
                self.tenths.insert(*step, tenth);
                Some(JobLine::new(
                    Level::DEBUG,
                    Some(*step),
                    format!("{done} of {total} done"),
                ))
            }
            Progress::StepMessage { step, text } => info(Some(*step), text.clone()),
            Progress::StepFinished { step, measure } => {
                let mut line = format!("Step finished in {:.1} s", measure.wall_s);
                if let Some(mib) = measure.peak_ram_mib {
                    line.push_str(&format!(", {mib:.0} MiB RAM"));
                }
                if let Some(mib) = measure.peak_vram_mib {
                    line.push_str(&format!(", {mib:.0} MiB VRAM"));
                }
                for (key, value) in &measure.notes {
                    line.push_str(&format!(", {key}={value}"));
                }
                info(Some(*step), line)
            }
            Progress::StepFailed { step, message } => Some(JobLine::new(
                Level::ERROR,
                Some(*step),
                format!("Step failed: {message}"),
            )),
            Progress::ModelCall { .. } => None,
        }
    }
}

/// The line for a job's end, naming the `localized` video when the job wrote one.
pub(crate) fn describe_end(
    outcome: &Result<JobOutcome, PipelineError>,
    localized: Option<&Path>,
) -> JobLine {
    match outcome {
        Ok(done) => JobLine::new(
            Level::INFO,
            None,
            format!(
                "Job finished: {} steps ran, {} kept; subtitles {}{}",
                done.ran.len(),
                done.skipped.len(),
                done.subtitles.display(),
                localized.map_or_else(String::new, |path| format!(
                    "; localized video {}",
                    path.display()
                ))
            ),
        ),
        Err(error) => JobLine::new(Level::ERROR, None, format!("Job stopped: {error}")),
    }
}

/// Log `line` under the `job` target, with the video's `name` and its step as fields.
pub(crate) fn emit(name: &str, line: &JobLine) {
    let step = line.step.map(|step| step.to_string());
    let step = step.as_deref();
    let text = &line.text;
    match line.level {
        Level::ERROR => tracing::error!(target: "job", video = name, step, "{text}"),
        Level::WARN => tracing::warn!(target: "job", video = name, step, "{text}"),
        Level::INFO => tracing::info!(target: "job", video = name, step, "{text}"),
        Level::DEBUG => tracing::debug!(target: "job", video = name, step, "{text}"),
        Level::TRACE => tracing::trace!(target: "job", video = name, step, "{text}"),
    }
}

/// Pass a worker's model call on as the exchange event the log window keeps, in the caller's
/// span.
pub(crate) fn emit_call(call: &ModelExchange) {
    if tracing::enabled!(target: EXCHANGE_TARGET, Level::TRACE)
        && let Ok(json) = serde_json::to_string(call)
    {
        tracing::trace!(target: EXCHANGE_TARGET, exchange = %json);
    }
}

fn minutes(secs: f64) -> String {
    format!("{:.1} min", secs / 60.0)
}

#[cfg(test)]
#[path = "tests/progress_log.rs"]
mod tests;

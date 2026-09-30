//! A worker's frames as the runner reads them: progress events for the caller, and what the
//! worker says about its own end.
//!
//! **Role:** read the frames of a worker's stdout as bytes, turn `Progress` and `ModelCall` frames
//! into [`Progress`] events, and collect its `Measure`, its `Failed` message and its `Done`.
//!
//! **Position:** called by `run_worker` on the worker's stdout pipe; uses `worker_channel` for the
//! codec and `job_model` for the model call and the measure.
//!
//! **Signals and state:** reads the pipe until it ends; calls the progress sink per frame.
//!
//! **Invariants:** a frame the runner never takes from a worker (`Input`, `Output`), a measure that
//! does not check, any frame after `Done`, a stream cut inside a frame and an unknown tag are
//! protocol errors; a model call that does not parse is a short message, never its bytes; a
//! `Failed` message that is not UTF-8 is kept, its bad bytes replaced.

use std::io::Read;

use job_model::StepName;
use job_model::job::WorkerMeasure;
use job_model::model_call::ModelExchange;
use worker_channel::frame::{self, Tag};
use worker_channel::progress::Progress as ChannelProgress;

use crate::progress::{Progress, ProgressSink};

/// What a worker said about its own end.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct WorkerReport {
    /// Its measure of itself, once sent.
    pub(crate) measure: Option<WorkerMeasure>,
    /// Why it failed, when it said so.
    pub(crate) failed: Option<String>,
    /// Whether it sent `Done`.
    pub(crate) done: bool,
}

/// Read `step`'s worker frames from `stdout` until it ends; `Err` names the protocol break.
pub(crate) fn read_frames(
    step: StepName,
    stdout: &mut impl Read,
    progress: ProgressSink,
) -> Result<WorkerReport, String> {
    let mut report = WorkerReport::default();
    loop {
        let frame = match frame::read_frame(stdout) {
            Ok(Some(frame)) => frame,
            Ok(None) => return Ok(report),
            Err(error) => return Err(format!("the worker's frames broke off: {error}")),
        };
        if report.done {
            return Err(format!(
                "the worker sent a {} frame after its end",
                frame.tag.name()
            ));
        }
        match frame.tag {
            Tag::Progress => {
                let advance = ChannelProgress::decode(&frame.payload)
                    .map_err(|error| format!("the worker's progress could not be read: {error}"))?;
                progress(Progress::StepAdvanced {
                    step,
                    done: usize::try_from(advance.done).unwrap_or(usize::MAX),
                    total: usize::try_from(advance.total).unwrap_or(usize::MAX),
                });
            }
            Tag::ModelCall => progress(model_call(step, &frame.payload)),
            Tag::Measure => {
                let measure =
                    rkyv::from_bytes::<WorkerMeasure, rkyv::rancor::Error>(&frame.payload)
                        .map_err(|error| {
                            format!("the worker's measure could not be read: {error}")
                        })?;
                report.measure = Some(measure);
            }
            Tag::Failed => {
                report.failed = Some(String::from_utf8_lossy(&frame.payload).into_owned());
            }
            Tag::Done => report.done = true,
            Tag::Input => {
                return Err("the worker sent an input, which only the runner sends".to_string());
            }
            Tag::Output => {
                return Err("the worker sent an output, which no step stores yet".to_string());
            }
        }
    }
}

/// A `ModelCall` payload as a progress event: the call, or a short message when it does not parse.
fn model_call(step: StepName, payload: &[u8]) -> Progress {
    match serde_json::from_slice::<ModelExchange>(payload) {
        Ok(call) => Progress::ModelCall {
            step,
            call: Box::new(call),
        },
        Err(_) => Progress::StepMessage {
            step,
            text: format!(
                "a model call that could not be read ({} bytes)",
                payload.len()
            ),
        },
    }
}

#[cfg(test)]
#[path = "tests/frames.rs"]
mod tests;

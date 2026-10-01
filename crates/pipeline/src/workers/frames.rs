//! A worker's frames as the runner reads them: progress events for the caller, outputs for the
//! step's write, and what the worker says about its own end.
//!
//! **Role:** read the frames of a worker's stdout as bytes, turn `Progress`, `ModelCall` and
//! `Message` frames into [`Progress`] events, hand each `Output` to the step's [`StepWrite`] straight from the
//! pipe, collect its `Measure`, its `Failed` message and its `Done`, and decide from them and the
//! exit code whether the step finished.
//!
//! **Position:** called by `run_worker` on the worker's stdout pipe; uses `worker_channel` for the
//! codec, `channel::outputs` to keep outputs and `job_model` for the model call and the measure.
//!
//! **Signals and state:** reads the pipe until it ends; calls the progress sink per frame.
//!
//! **Invariants:** an `Input`, an `Output` with no step write to take it, an output the step write
//! refuses, a frame shorter than its address, a measure that does not check, any frame after
//! `Done`, a stream cut inside a frame and an unknown tag are protocol errors; an `Output`'s
//! archive is never buffered here; a model call that does not parse is a short message, never its
//! bytes; a `Failed` or `Message` text that is not UTF-8 is kept, its bad bytes replaced.

use std::io::Read;

use job_model::StepName;
use job_model::job::WorkerMeasure;
use job_model::model_call::ModelExchange;
use worker_channel::address::Address;
use worker_channel::frame::{self, Tag};
use worker_channel::progress::Progress as ChannelProgress;

use super::channel::StepWrite;
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

impl WorkerReport {
    /// The worker's measure when it finished: exit code 0, its `Measure` and its `Done`. `Err`
    /// says why not, after the worker's own `Failed` message when it sent one.
    pub(crate) fn verdict(self, code: i32) -> Result<WorkerMeasure, String> {
        let failed = self
            .failed
            .as_ref()
            .map(|message| format!("{message}\n"))
            .unwrap_or_default();
        let ending = if code != 0 {
            format!("the worker exited {code}")
        } else {
            match (&self.measure, self.done) {
                (Some(_), true) => String::new(),
                (None, true) => "the worker exited 0 without sending its measure".to_string(),
                (Some(_), false) => "the worker exited 0 without sending its end".to_string(),
                (None, false) => {
                    "the worker exited 0 without sending its measure or its end".to_string()
                }
            }
        };
        match self.measure {
            Some(measure) if ending.is_empty() => Ok(measure),
            _ => Err(format!("{failed}{ending}")),
        }
    }
}

/// Read `step`'s worker frames from `stdout` until it ends, handing each output to `outputs`;
/// `Err` names the protocol break.
pub(crate) fn read_frames(
    step: StepName,
    stdout: &mut impl Read,
    progress: ProgressSink,
    mut outputs: Option<&mut StepWrite>,
) -> Result<WorkerReport, String> {
    let mut report = WorkerReport::default();
    loop {
        let header = match frame::read_header(stdout) {
            Ok(Some(header)) => header,
            Ok(None) => return Ok(report),
            Err(error) => return Err(format!("the worker's frames broke off: {error}")),
        };
        if report.done {
            return Err(format!(
                "the worker sent a {} frame after its end",
                header.tag.name()
            ));
        }
        let payload = match header.tag {
            Tag::Output => {
                let Some(outputs) = outputs.as_deref_mut() else {
                    return Err("the worker sent an output, which no step stores yet".to_string());
                };
                receive(stdout, header.len, outputs)?;
                continue;
            }
            Tag::Input => {
                return Err("the worker sent an input, which only the runner sends".to_string());
            }
            _ => frame::read_payload(stdout, header.len)
                .map_err(|error| format!("the worker's frames broke off: {error}"))?,
        };
        match header.tag {
            Tag::Progress => {
                let advance = ChannelProgress::decode(&payload)
                    .map_err(|error| format!("the worker's progress could not be read: {error}"))?;
                progress(Progress::StepAdvanced {
                    step,
                    done: usize::try_from(advance.done).unwrap_or(usize::MAX),
                    total: usize::try_from(advance.total).unwrap_or(usize::MAX),
                });
            }
            Tag::ModelCall => progress(model_call(step, &payload)),
            Tag::Message => progress(Progress::StepMessage {
                step,
                text: String::from_utf8_lossy(&payload).into_owned(),
            }),
            Tag::Measure => {
                let measure = rkyv::from_bytes::<WorkerMeasure, rkyv::rancor::Error>(&payload)
                    .map_err(|error| format!("the worker's measure could not be read: {error}"))?;
                report.measure = Some(measure);
            }
            Tag::Failed => {
                report.failed = Some(String::from_utf8_lossy(&payload).into_owned());
            }
            Tag::Done => report.done = true,
            // Answered before any payload is read.
            Tag::Input | Tag::Output => {}
        }
    }
}

/// Read one `Output` frame of `len` bytes: its address, then its archive straight into `outputs`,
/// which must take exactly the rest of the frame.
fn receive(stdout: &mut impl Read, len: u32, outputs: &mut StepWrite) -> Result<(), String> {
    let mut frame = stdout.take(u64::from(len));
    let (address, taken) = Address::read(&mut frame).map_err(|error| {
        format!("the worker sent an output whose address could not be read: {error}")
    })?;
    let archive_len = usize::try_from(frame.limit())
        .map_err(|_| "the worker sent an output too large for this machine".to_string())?;
    debug_assert_eq!(archive_len + taken, len as usize);
    outputs.receive(address, archive_len, &mut frame)?;
    if frame.limit() != 0 {
        return Err(format!(
            "the worker's output left {} bytes of its frame unread",
            frame.limit()
        ));
    }
    Ok(())
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

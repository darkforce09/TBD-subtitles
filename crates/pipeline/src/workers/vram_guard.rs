//! The wait for GPU memory: a GPU step that holds the GPU lock starts its model only once the
//! card has the memory the step was measured to need, and fails, saying why, when it never does.
//!
//! **Role:** read the device's free memory every second until it reaches the step's need from
//! `graph::vram_need_mib`; say once, and again on each meaningful change, how much is free and
//! how much is needed; give up when cancelled or after [`DEADLINE`].
//!
//! **Position:** called by `workers::run_worker` right after the GPU lock is taken, and by
//! `workers::lazy_gpu` in a worker that takes the lock only when its model loads; reads NVML
//! through `measure::gpu_monitor`, behind [`FreeMemory`] so tests use a fake.
//!
//! **Signals and state:** polls the free-memory source and sleeps; calls the message sink.
//!
//! **Invariants:** without NVML there is no check; a wait never outlasts [`DEADLINE`] and never
//! ends silently: it returns once enough memory is free, a cancelled error when cancelled, or an
//! error naming the free memory, the need and what to do; a step without a need never waits.

use std::time::{Duration, Instant};

use job_model::StepName;

use crate::cancel::CancelToken;
use crate::error::{PipelineError, Result};
use crate::graph;
use crate::measure::gpu_monitor;

/// How often the free memory is read while a step waits.
pub const POLL: Duration = Duration::from_secs(1);
/// How long a step waits for its memory before it fails.
pub const DEADLINE: Duration = Duration::from_secs(10 * 60);
/// A change in free memory, in MiB, worth a new waiting line.
const MESSAGE_STEP_MIB: u64 = 128;
/// How often a sleeping wait looks at its cancel token.
const CANCEL_SLICE: Duration = Duration::from_millis(100);

/// Where the device's free memory is read from.
pub trait FreeMemory {
    /// The device's free memory now, in MiB; `None` when it cannot be read.
    fn free_mib(&mut self) -> Option<u64>;
}

/// The first GPU's free memory, read through NVML.
#[derive(Debug, Clone, Copy, Default)]
pub struct NvmlFreeMemory;

impl FreeMemory for NvmlFreeMemory {
    fn free_mib(&mut self) -> Option<u64> {
        gpu_monitor::device_memory().map(|memory| memory.free_mib)
    }
}

/// How often to look and how long to wait.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timing {
    pub poll: Duration,
    pub deadline: Duration,
}

impl Default for Timing {
    fn default() -> Timing {
        Timing {
            poll: POLL,
            deadline: DEADLINE,
        }
    }
}

/// Wait until the GPU has `step`'s memory need free, read through NVML, saying while it waits
/// through `say`.
pub fn wait_for_memory(step: StepName, cancel: &CancelToken, say: &dyn Fn(String)) -> Result<()> {
    let Some(need_mib) = graph::vram_need_mib(step) else {
        return Ok(());
    };
    wait_with(
        step,
        need_mib,
        &mut NvmlFreeMemory,
        Timing::default(),
        cancel,
        say,
    )
}

/// Wait until `memory` reports `need_mib` free for `step`, polling at `timing.poll`, saying
/// through `say` the first time it is short and whenever the free memory moves by
/// [`MESSAGE_STEP_MIB`] or more since the last line, and failing after `timing.deadline`.
pub fn wait_with(
    step: StepName,
    need_mib: u64,
    memory: &mut dyn FreeMemory,
    timing: Timing,
    cancel: &CancelToken,
    say: &dyn Fn(String),
) -> Result<()> {
    let context = format!("step {step}");
    let started = Instant::now();
    let mut told: Option<u64> = None;
    loop {
        if cancel.is_cancelled() {
            return Err(PipelineError::cancelled(context));
        }
        let Some(free_mib) = memory.free_mib() else {
            tracing::debug!("step {step}: NVML cannot be read, so GPU memory is not checked");
            return Ok(());
        };
        if free_mib >= need_mib {
            if told.is_some() {
                tracing::debug!(
                    "step {step}: {free_mib} MiB of GPU memory free, {need_mib} needed"
                );
            }
            return Ok(());
        }
        if started.elapsed() >= timing.deadline {
            return Err(PipelineError::new(
                context,
                deadline_message(free_mib, need_mib, timing.deadline),
            ));
        }
        if told.is_none_or(|last| last.abs_diff(free_mib) >= MESSAGE_STEP_MIB) {
            say(waiting_message(free_mib, need_mib));
            told = Some(free_mib);
        }
        sleep(timing.poll, cancel);
    }
}

/// The line a step shows while it waits for memory.
pub fn waiting_message(free_mib: u64, need_mib: u64) -> String {
    format!("waiting for GPU memory: {free_mib} MiB free, {need_mib} needed")
}

/// Why a step that waited `waited` for its memory failed.
fn deadline_message(free_mib: u64, need_mib: u64, waited: Duration) -> String {
    format!(
        "only {free_mib} MiB of GPU memory was free after {} of waiting, and the step needs \
         {need_mib} MiB; close other GPU programs and retry",
        waited_words(waited)
    )
}

/// `waited` in words: whole minutes, or seconds below one.
fn waited_words(waited: Duration) -> String {
    match waited.as_secs() {
        60 => "1 minute".to_string(),
        secs if secs >= 60 => format!("{} minutes", secs / 60),
        secs => format!("{secs} s"),
    }
}

/// Sleep for `span`, waking early when `cancel` is set.
fn sleep(span: Duration, cancel: &CancelToken) {
    let until = Instant::now() + span;
    while !cancel.is_cancelled() {
        let left = until.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return;
        }
        std::thread::sleep(left.min(CANCEL_SLICE));
    }
}

#[cfg(test)]
#[path = "tests/vram_guard.rs"]
mod tests;

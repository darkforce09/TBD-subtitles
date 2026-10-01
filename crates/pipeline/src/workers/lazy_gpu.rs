//! The GPU taken late: a worker whose step locks the GPU only when it loads its model takes the
//! lock and the memory wait just before that load, and keeps both for as long as the model lives.
//!
//! **Role:** name the lock file to such a worker ([`LOCK_VARIABLE`], set by `run_worker`), and in
//! the worker take that lock with the step's priority, wait for the step's memory, open the model
//! and wrap it in a [`HeldModel`] that owns the hold, telling the runner while it waits through
//! `Message` frames.
//!
//! **Position:** `run_worker` sets the variable for steps where `graph::locks_gpu_lazily`; the
//! translation task in `tasks::onscreen` calls [`open_held`] where it opens its local model; uses
//! `gpu_lock`, `vram_guard` and `worker_channel::worker::message`.
//!
//! **Signals and state:** reads [`LOCK_VARIABLE`]; takes the GPU lock file's `flock` and polls
//! NVML; sends `Message` frames when the worker channel is installed.
//!
//! **Invariants:** the model is opened only once the lock is held and its memory is free; the
//! model drops before the lock is released; a worker with no lock named takes none; the wait is
//! never cancelled from inside, because the runner stops the worker by killing it, which frees
//! the `flock`.

use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use inference::llm::{Completion, LanguageModel, LlmError};
use job_model::StepName;

use super::gpu_lock::{self, GpuLock, Holder};
use super::vram_guard::{self, FreeMemory, NvmlFreeMemory, Timing};
use crate::cancel::CancelToken;
use crate::error::Result;
use crate::graph;

/// The variable that names the GPU lock file to a worker that takes the lock itself; unset for
/// every other worker.
pub const LOCK_VARIABLE: &str = "TBD_SUBTITLES_GPU_LOCK";

/// Why a model held on the GPU could not be opened: the lock or memory wait, or the open itself.
pub type OpenError = Box<dyn Error + Send + Sync>;

/// The variable `run_worker` adds for `step`, naming `gpu_lock`; `None` for a step that takes the
/// lock before its worker starts.
pub fn worker_variable(step: StepName, gpu_lock: &Path) -> Option<(String, String)> {
    graph::locks_gpu_lazily(step).then(|| {
        (
            LOCK_VARIABLE.to_string(),
            gpu_lock.to_string_lossy().into_owned(),
        )
    })
}

/// The lock file a variable's value names; `None` when it is unset or empty.
pub fn named_lock(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// The GPU lock a worker holds while its model lives.
#[derive(Debug)]
pub struct GpuHold {
    _lock: GpuLock,
}

/// Take the lock at `path` for `step` of the job at `job`, with the priority the graph gives the
/// step, then wait for the step's memory as `memory` reports it, saying through `say` while it
/// waits.
pub fn hold(
    path: &Path,
    step: StepName,
    job: &Path,
    memory: &mut dyn FreeMemory,
    say: &dyn Fn(String),
) -> Result<GpuHold> {
    let never = CancelToken::new();
    let holder = Holder {
        step,
        job: job.to_path_buf(),
    };
    let lock = gpu_lock::acquire(path, holder, graph::gpu_priority(step), &never, &|held| {
        say(waiting_line(held, job))
    })?;
    if let Some(need_mib) = graph::vram_need_mib(step) {
        vram_guard::wait_with(step, need_mib, memory, Timing::default(), &never, say)?;
    }
    Ok(GpuHold { _lock: lock })
}

/// What a worker shows while another holder has the GPU: the holder's step when it is one of
/// this process's, else another step, of this app or another run of it.
pub fn waiting_line(held: Option<&Holder>, job: &Path) -> String {
    match held {
        Some(_) => gpu_lock::waiting_message(held, job),
        None => "waiting for the GPU: another step is using it".to_string(),
    }
}

/// Open `step`'s model with `open` once the GPU lock named in [`LOCK_VARIABLE`] is held and the
/// step's memory is free, read through NVML, telling the runner while it waits; without a lock
/// named, open it at once.
pub fn open_held<M, E>(
    step: StepName,
    job: &Path,
    open: impl FnOnce() -> std::result::Result<M, E>,
) -> std::result::Result<HeldModel<M>, OpenError>
where
    E: Into<OpenError>,
{
    let lock = named_lock(std::env::var_os(LOCK_VARIABLE));
    open_held_with(
        lock.as_deref(),
        step,
        job,
        &mut NvmlFreeMemory,
        &tell_runner,
        open,
    )
}

/// [`open_held`] with the lock file, the free-memory source and the message sink given.
pub fn open_held_with<M, E>(
    lock: Option<&Path>,
    step: StepName,
    job: &Path,
    memory: &mut dyn FreeMemory,
    say: &dyn Fn(String),
    open: impl FnOnce() -> std::result::Result<M, E>,
) -> std::result::Result<HeldModel<M>, OpenError>
where
    E: Into<OpenError>,
{
    let held = match lock {
        Some(path) => Some(hold(path, step, job, memory, say)?),
        None => None,
    };
    let model = open().map_err(Into::into)?;
    Ok(HeldModel { model, hold: held })
}

/// Send a waiting line to the runner as a `Message` frame.
fn tell_runner(line: String) {
    tracing::info!("{line}");
    worker_channel::worker::message(&line);
}

/// A model and the GPU lock it was loaded under; the model drops first, then the lock.
pub struct HeldModel<M> {
    // Declared before the hold, so it drops before the lock is released.
    model: M,
    hold: Option<GpuHold>,
}

impl<M> HeldModel<M> {
    /// Whether the model holds the GPU lock.
    pub fn holds_gpu(&self) -> bool {
        self.hold.is_some()
    }
}

impl<M> std::fmt::Debug for HeldModel<M> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HeldModel")
            .field("holds_gpu", &self.holds_gpu())
            .finish_non_exhaustive()
    }
}

impl<M: LanguageModel> LanguageModel for HeldModel<M> {
    fn name(&self) -> String {
        self.model.name()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        schema: &serde_json::Value,
    ) -> std::result::Result<Completion, LlmError> {
        self.model.complete_json(system, user, schema)
    }
}

#[cfg(test)]
#[path = "tests/lazy_gpu.rs"]
mod tests;

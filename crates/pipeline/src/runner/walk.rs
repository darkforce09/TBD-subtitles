//! One step's run, the same wherever it runs: on the main walk, on the shot scan's thread or in
//! the visual lane.
//!
//! **Role:** skip a step that is still valid, or take its fingerprint, forget its record, run it,
//! commit its outputs with its record, and tell the listener each of these; keep the first real
//! failure of the job, whichever thread hit it, and stop the job's other steps when one fails.
//!
//! **Position:** used by `runner` (`run_job`) and `runner::lane`; calls `resume`, `rerun` and the
//! runner's `measured` step function, and commits through `workers::StepWrite`.
//!
//! **Signals and state:** emits `StepSkipped`, `StepStarted`, `StepFinished`, `StepFailed` and
//! `JobDuration`; writes step records; sets the job's cancel token when a step fails; holds the
//! job's first failure behind a mutex shared by the runner's threads.
//!
//! **Invariants:** a step's record is committed with its outputs, and removed before the step
//! runs again; a step that fails reports `StepFailed` once, and a step stopped only because
//! another step failed reports none; the job ends with its first real failure, not with the stop
//! that failure caused.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::ScopedJoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};

use job_model::StepName;
use job_model::job::{JobRecord, StepMeasure, StepRecord};
use job_model::outputs::ProbeDecoded;

use crate::cancel::CancelToken;
use crate::error::{PipelineError, Result};
use crate::library::Library;
use crate::progress::{Progress, ProgressSink};
use crate::resume;
use crate::work_dir::store::StoreRead;
use crate::work_dir::{JobStore, WorkDir};
use crate::workers::StepWrite;

use super::rerun;

/// A step's measure, and the outputs it wrote, uncommitted.
pub(super) type Measured<'a> =
    dyn Fn(StepName, &JobRecord) -> Result<(StepMeasure, Option<StepWrite>)> + Sync + 'a;

/// What the runner's threads share to run steps.
pub(super) struct Steps<'a> {
    pub(super) store: &'a Arc<JobStore>,
    pub(super) record: &'a JobRecord,
    pub(super) work: &'a WorkDir,
    pub(super) library: Option<&'a Library>,
    pub(super) measured: &'a Measured<'a>,
    pub(super) progress: ProgressSink<'a>,
    pub(super) cancel: &'a CancelToken,
    /// The job's first real failure.
    failure: Mutex<Option<PipelineError>>,
}

/// How a step began.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Begun {
    /// Still valid: skipped.
    Skipped,
    /// To run, its record forgotten, with the fingerprint taken before it runs.
    Started { fingerprint: String },
}

/// The steps a walk ran and skipped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Tally {
    pub(super) ran: Vec<StepName>,
    pub(super) skipped: Vec<StepName>,
}

impl Tally {
    /// Add `other`'s steps.
    pub(super) fn merge(&mut self, other: Tally) {
        self.ran.extend(other.ran);
        self.skipped.extend(other.skipped);
    }

    /// The steps run and skipped, each in `StepName::ALL` order.
    pub(super) fn in_run_order(mut self) -> (Vec<StepName>, Vec<StepName>) {
        let position = |step: &StepName| StepName::ALL.iter().position(|s| s == step);
        self.ran.sort_by_key(position);
        self.skipped.sort_by_key(position);
        (self.ran, self.skipped)
    }
}

impl<'a> Steps<'a> {
    pub(super) fn new(
        store: &'a Arc<JobStore>,
        record: &'a JobRecord,
        library: Option<&'a Library>,
        measured: &'a Measured<'a>,
        progress: ProgressSink<'a>,
        cancel: &'a CancelToken,
    ) -> Steps<'a> {
        Steps {
            store,
            record,
            work: store.work(),
            library,
            measured,
            progress,
            cancel,
            failure: Mutex::new(None),
        }
    }

    /// A cancelled error when the job is stopping, before `step` starts.
    pub(super) fn check_cancel(&self, step: StepName) -> Result<()> {
        if self.cancel.is_cancelled() {
            return Err(PipelineError::cancelled(format!("step {step}")));
        }
        Ok(())
    }

    /// Run `step` from beginning to end, unless the job is stopping.
    pub(super) fn run(&self, step: StepName, tally: &mut Tally) -> Result<()> {
        self.check_cancel(step)?;
        match self.begin(step, tally)? {
            Begun::Skipped => Ok(()),
            Begun::Started { fingerprint } => self.complete(step, fingerprint),
        }
    }

    /// Skip `step` while it is valid; else take its fingerprint, forget its record and tell the
    /// listener it starts.
    pub(super) fn begin(&self, step: StepName, tally: &mut Tally) -> Result<Begun> {
        let read = self.store.read()?;
        if resume::is_valid(step, self.record, &read, self.work, self.library) {
            tally.skipped.push(step);
            (self.progress)(Progress::StepSkipped(step));
            announce_duration(step, &read, self.progress);
            return Ok(Begun::Skipped);
        }
        let fingerprint = resume::fingerprint(step, self.record, &read, self.library)?;
        drop(read);
        rerun::forget(self.store, step)?;
        tally.ran.push(step);
        (self.progress)(Progress::StepStarted(step));
        Ok(Begun::Started { fingerprint })
    }

    /// Run a started `step` and commit its outputs with its record.
    pub(super) fn complete(&self, step: StepName, fingerprint: String) -> Result<()> {
        let stamped = (self.measured)(step, self.record)
            .and_then(|(measure, outputs)| stamp(self.store, step, fingerprint, measure, outputs))
            .inspect_err(|error| self.failed(step, error))?;
        (self.progress)(Progress::StepFinished {
            step,
            measure: stamped.measure,
        });
        announce_duration(step, &self.store.read()?, self.progress);
        Ok(())
    }

    /// Report `step`'s failure, unless it only stopped because another step failed, and keep it
    /// as the job's failure when it is the first.
    pub(super) fn failed(&self, step: StepName, error: &PipelineError) {
        if reports_failure(error, self.failure().is_some()) {
            (self.progress)(Progress::StepFailed {
                step,
                message: error.to_string(),
            });
        }
        self.note_failure(error);
    }

    /// Keep `error` as the job's failure when it is a real one and the first, and stop the job's
    /// other steps.
    pub(super) fn note_failure(&self, error: &PipelineError) {
        if error.is_cancelled() {
            return;
        }
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get_or_insert_with(|| error.clone());
        self.cancel.cancel();
    }

    /// The job's first real failure, if a step had one.
    pub(super) fn failure(&self) -> Option<PipelineError> {
        self.failure
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// The error a walk that ended with `error` returns: the first real failure when `error` is
    /// only the stop it caused.
    pub(super) fn job_error(&self, error: PipelineError) -> PipelineError {
        job_error(error, self.failure())
    }

    /// Wait for the shot scan's thread; a thread that panicked is a failed scan.
    pub(super) fn join_shots(&self, handle: ScopedJoinHandle<'_, Result<()>>) -> Result<()> {
        handle.join().unwrap_or_else(|_| {
            let error = PipelineError::new("step shot_scan", "the scan thread panicked");
            self.failed(StepName::ShotScan, &error);
            Err(error)
        })
    }
}

/// Whether a step that ended with `error` reports `StepFailed`: always for a real failure; for a
/// stop, only when no step failed first, since then the owner stopped the job.
pub(super) fn reports_failure(error: &PipelineError, failed_before: bool) -> bool {
    !error.is_cancelled() || !failed_before
}

/// `first`, when `error` is a stop and a step failed first; else `error`.
pub(super) fn job_error(error: PipelineError, first: Option<PipelineError>) -> PipelineError {
    match first {
        Some(first) if error.is_cancelled() => first,
        _ => error,
    }
}

/// Tell the listener the video's length once the probe is there.
pub(super) fn announce_duration(step: StepName, read: &StoreRead, progress: ProgressSink) {
    if step == StepName::ProbeDecode
        && let Ok(Some(probe)) = read.output::<ProbeDecoded>(StepName::ProbeDecode, None)
    {
        progress(Progress::JobDuration(probe.probe.duration_s));
    }
}

/// `step`'s record, with the fingerprint captured before it ran, so concurrent edits remain stale;
/// the outputs the step wrote are committed with it, in one transaction.
fn stamp(
    store: &Arc<JobStore>,
    step: StepName,
    fingerprint: String,
    measure: StepMeasure,
    outputs: Option<StepWrite>,
) -> Result<StepRecord> {
    let stamped = StepRecord {
        fingerprint,
        finished_ns: now_ns(),
        measure,
    };
    outputs
        .unwrap_or_else(|| StepWrite::new(store.clone()))
        .commit(step, &stamped)?;
    Ok(stamped)
}

/// Now, in nanoseconds since the Unix epoch.
pub(super) fn now_ns() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

#[cfg(test)]
#[path = "tests/walk.rs"]
mod tests;

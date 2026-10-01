//! The job runner: one video through every step, skipping what is still valid, the shot scan
//! alongside the GPU steps, each step's measure recorded, and the report written at the end.
//!
//! **Role:** open or create the job's work directory and database, put this run's job record and
//! clear the steps asked to run again, walk `StepName::ALL`, run each stale step in process or in
//! its worker, and commit its outputs with its record.
//!
//! **Position:** called by the app's `process` subcommand and its window; uses `resume`,
//! `workers`, `tasks`, `report` and `rerun`.
//!
//! **Signals and state:** holds the job's `JobStore` (and so `job.redb` and `job.lock`) until the
//! run returns; sends each worker the stored values its step reads; commits the outputs a step
//! wrote with its step record, in one transaction, for in-process and worker steps alike; emits
//! progress events.
//!
//! **Invariants:** a step record exists only beside the outputs it was committed with, and a step
//! about to run again loses its record first, so a killed job resumes from the last finished step;
//! one worker loads the GPU at a time (the shot scan uses none); a step starts only after every
//! step it reads has finished.

pub mod rerun;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use inference::cuda_runtime::CudaRuntime;
use job_model::StepName;
use job_model::job::{JobRecord, JobSettings, StepMeasure, StepRecord};
use job_model::outputs::ProbeDecoded;
use job_model::report::QcReport;
use worker_channel::address::Address;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Placement};
use crate::progress::{Progress, ProgressSink};
use crate::tasks::{self, Job, StepIo};
use crate::work_dir::store::StoreRead;
use crate::work_dir::{self, JobStore, WorkDir};
use crate::workers::{self, Binaries, StepWrite, WorkerData};
use crate::{report, resume};

/// How to run a job.
#[derive(Debug, Clone)]
pub struct JobOptions {
    /// The folder that holds every job's work directory.
    pub work_root: PathBuf,
    pub settings: JobSettings,
    /// Steps to run again even when their output is valid; the steps after them follow.
    pub rerun: Vec<StepName>,
    pub binaries: Binaries,
    /// The folder the models are read from.
    pub models_dir: PathBuf,
    /// Stops the job: no further step starts and the running worker is killed. The runner also
    /// sets it when a step fails, so the shot scan running alongside stops too.
    pub cancel: CancelToken,
    /// The machine-wide GPU lock file (`gpu.lock` in the app data folder).
    pub gpu_lock: PathBuf,
}

/// What a finished job left.
#[derive(Debug, Clone)]
pub struct JobOutcome {
    pub work_dir: PathBuf,
    pub subtitles: PathBuf,
    pub report: PathBuf,
    pub qc: QcReport,
    pub ran: Vec<StepName>,
    pub skipped: Vec<StepName>,
}

/// Run every step of `video`'s job.
pub fn run_job(video: &Path, options: &JobOptions, progress: ProgressSink) -> Result<JobOutcome> {
    let video = fs::canonicalize(video).context(format!("cannot find {}", video.display()))?;
    let meta = fs::metadata(&video).context(format!("cannot read {}", video.display()))?;
    if !meta.is_file() {
        return Err(PipelineError::new(
            video.display().to_string(),
            "not a file",
        ));
    }
    let modified_s = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs() as i64);
    let work = WorkDir::new(options.work_root.join(work_dir::job_id(&video)));
    // Another process running this job makes the open fail with the busy kind.
    let store = JobStore::open(&work)?;
    let record = JobRecord {
        video: video.to_string_lossy().into_owned(),
        video_size: meta.len(),
        video_modified_s: modified_s,
        settings: options.settings.clone(),
        models_dir: Some(options.models_dir.to_string_lossy().into_owned()),
        corrections: work_dir::corrections_digest(&store)?,
    };
    tracing::debug!("job {} for {}", work.root().display(), video.display());
    let cleared = rerun::start(&store, &record, &options.rerun)?;
    if !options.rerun.is_empty() {
        tracing::debug!(
            "rerunning on request: {:?}, clearing {cleared:?}",
            options.rerun
        );
    }
    progress(Progress::JobStarted {
        video: video.clone(),
        work_dir: work.root().to_path_buf(),
        stale: resume::stale_steps(&record, &store.read()?, &work),
    });

    let cuda_env: OnceLock<std::result::Result<Vec<(String, String)>, String>> = OnceLock::new();
    let env_for = |step: StepName| -> Result<Vec<(String, String)>> {
        if !graph::needs_cuda_runtime(step) {
            return Ok(Vec::new());
        }
        cuda_env
            .get_or_init(|| {
                let located = locate_cuda(&options.binaries).map_err(|e| e.to_string());
                match &located {
                    Ok(_) => tracing::debug!("the CUDA runtime is found for GPU workers"),
                    Err(error) => tracing::warn!("no CUDA runtime for GPU workers: {error}"),
                }
                located
            })
            .clone()
            .map_err(|e| PipelineError::new(format!("step {step}"), e))
    };
    // A step's measure, and the outputs its worker sent, uncommitted.
    let run_step =
        |step: StepName, record: &JobRecord| -> Result<(StepMeasure, Option<StepWrite>)> {
            let placement = if !record.settings.onscreen_text.enabled
                && step.stage() == job_model::StageName::OnscreenText
            {
                Placement::InProcess
            } else {
                graph::placement(step)
            };
            match placement {
                Placement::InProcess => {
                    tracing::debug!("step {step} runs in this process");
                    let job = Job {
                        work: work.clone(),
                        record: record.clone(),
                    };
                    let mut io = StepIo::in_process(&store)?;
                    let measure = tasks::in_process(step, &job, &mut io, &|done, total| {
                        progress(Progress::StepAdvanced { step, done, total })
                    })?;
                    Ok((measure, io.into_outputs()))
                }
                Placement::Worker(binary) => {
                    tracing::debug!(
                        "step {step} runs in a worker of {}",
                        options.binaries.path(binary).display()
                    );
                    let env = env_for(step)?;
                    let inputs = worker_inputs(step, &store.read()?)?;
                    let data = WorkerData {
                        store: &store,
                        inputs: &inputs,
                    };
                    let run = workers::run_worker(
                        options.binaries.path(binary),
                        step,
                        data,
                        &env,
                        progress,
                        &options.cancel,
                        &options.gpu_lock,
                    )?;
                    Ok((run.measure, run.outputs))
                }
            }
        };

    let (mut ran, mut skipped) = (Vec::new(), Vec::new());
    std::thread::scope(|scope| -> Result<()> {
        let mut shots = None;
        let walked = (|| -> Result<()> {
            for step in StepName::ALL {
                if options.cancel.is_cancelled() {
                    return Err(PipelineError::cancelled(format!("step {step}")));
                }
                // Everything this step logs, its workers' and programs' lines too, is under its span.
                let step_span = tracing::info_span!("step", step = %step);
                let _in_step = step_span.enter();
                if graph::inputs(step).contains(&StepName::ShotScan)
                    && let Some(handle) = shots.take()
                {
                    let scanned = join(handle)?;
                    finish(StepName::ShotScan, scanned, progress);
                }
                let read = store.read()?;
                if resume::is_valid(step, &record, &read, &work) {
                    skipped.push(step);
                    progress(Progress::StepSkipped(step));
                    announce_duration(step, &read, progress);
                    continue;
                }
                let fingerprint = resume::fingerprint(step, &record, &read)?;
                drop(read);
                rerun::forget(&store, step)?;
                ran.push(step);
                progress(Progress::StepStarted(step));
                if step == StepName::ShotScan {
                    let snapshot = record.clone();
                    let run_step = &run_step;
                    let store = &store;
                    let span = step_span.clone();
                    // The scan commits its own outputs, so a later step's outputs never wait on
                    // a transaction only this thread's join would end.
                    shots = Some(scope.spawn(move || {
                        let _in_step = span.enter();
                        let (measure, outputs) = run_step(StepName::ShotScan, &snapshot)?;
                        stamp(store, StepName::ShotScan, fingerprint, measure, outputs)
                    }));
                    continue;
                }
                let stamped = run_step(step, &record)
                    .and_then(|(measure, outputs)| {
                        stamp(&store, step, fingerprint, measure, outputs)
                    })
                    .inspect_err(|error| {
                        progress(Progress::StepFailed {
                            step,
                            message: error.to_string(),
                        })
                    })?;
                finish(step, stamped, progress);
                announce_duration(step, &store.read()?, progress);
            }
            if let Some(handle) = shots.take() {
                let scanned = join(handle)?;
                finish(StepName::ShotScan, scanned, progress);
            }
            Ok(())
        })();
        if walked.is_err() {
            // The scope waits for the shot scan; stop it rather than wait out a long scan.
            options.cancel.cancel();
        }
        walked
    })?;

    let steps = store.read()?.step_records()?;
    let qc = report::write(&work, &record, &steps)?;
    tracing::info!("the report is written to {}", work.report().display());
    Ok(JobOutcome {
        work_dir: work.root().to_path_buf(),
        subtitles: stages::output::subtitle_path(&video, record.settings.effective_output_format()),
        report: work.report(),
        qc,
        ran,
        skipped,
    })
}

/// Tell the listener the video's length once the probe is there.
fn announce_duration(step: StepName, read: &StoreRead, progress: ProgressSink) {
    if step == StepName::ProbeDecode
        && let Ok(Some(probe)) = read.output::<ProbeDecoded>(StepName::ProbeDecode, None)
    {
        progress(Progress::JobDuration(probe.probe.duration_s));
    }
}

/// The stored values `step`'s worker receives: every value it reads, less the optional ones the
/// job does not have.
pub fn worker_inputs(step: StepName, read: &StoreRead) -> Result<Vec<Address>> {
    let mut inputs = Vec::new();
    for address in graph::reads(step) {
        if graph::is_optional_read(step, &address)
            && read
                .with_bytes(address.table, &address.key, |_| Ok(()))?
                .is_none()
        {
            continue;
        }
        inputs.push(address);
    }
    Ok(inputs)
}

/// The CUDA runtime's environment for ONNX Runtime and Whisper workers, packaged beside the
/// binaries or in the runtime folder.
fn locate_cuda(binaries: &Binaries) -> Result<Vec<(String, String)>> {
    let runtime_dir =
        inference::model_store::runtime_dir().context("cannot find the runtime folder")?;
    let runtime =
        CudaRuntime::locate(binaries.main.parent(), &runtime_dir).context("CUDA runtime")?;
    Ok(runtime.worker_env())
}

fn join(handle: std::thread::ScopedJoinHandle<'_, Result<StepRecord>>) -> Result<StepRecord> {
    handle
        .join()
        .map_err(|_| PipelineError::new("step shot_scan", "the scan thread panicked"))?
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
    let finished_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let stamped = StepRecord {
        fingerprint,
        finished_ns,
        measure,
    };
    outputs
        .unwrap_or_else(|| StepWrite::new(store.clone()))
        .commit(step, &stamped)?;
    Ok(stamped)
}

/// Tell the listener `step` finished, with its measure; its record is already committed.
fn finish(step: StepName, stamped: StepRecord, progress: ProgressSink) {
    progress(Progress::StepFinished {
        step,
        measure: stamped.measure,
    });
}

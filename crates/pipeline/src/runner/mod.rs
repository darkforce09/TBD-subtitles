//! The job runner: one video through every step, skipping what is still valid, the shot scan
//! alongside the audio steps, the visual lane alongside adjudication, each step's measure
//! recorded, and the report written at the end.
//!
//! **Role:** open or create the job's work directory and database, put this run's job record and
//! clear the steps asked to run again, walk `StepName::ALL`, run each stale step in process or in
//! its worker, and commit its outputs with its record.
//!
//! **Position:** called by the app's `process` subcommand and its window; uses `resume`,
//! `workers`, `tasks`, `report` and `rerun`, `walk` for each step's run and `lane` for the visual
//! lane.
//!
//! **Signals and state:** holds the job's `JobStore` (and so `job.redb` and `job.lock`) until the
//! run returns; sends each worker the stored values its step reads; names the sign library to the
//! workers of the steps that read it; commits the outputs a step wrote with its step record, in
//! one transaction, for in-process and worker steps alike, with what the job's processes and the
//! GPU used while the step ran (`measure::job_sampler`); runs the shot scan and the visual lane
//! on scoped threads beside the main walk; puts the run's start, end and peak memory as
//! `meta/last_run` when the walk ends; emits progress events.
//!
//! **Invariants:** a step record exists only beside the outputs it was committed with, and a step
//! about to run again loses its record first, so a killed job resumes from the last finished step;
//! one worker loads the GPU at a time (the shot scan uses none); a step starts only after every
//! step it reads has finished; a failure on any thread stops the others and is the job's error.

mod lane;
pub mod rerun;
mod walk;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use inference::cuda_runtime::CudaRuntime;
use job_model::StepName;
use job_model::job::{JobRecord, JobRun, JobSettings, StepMeasure};
use job_model::report::QcReport;
use worker_channel::address::Address;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Placement};
use crate::library::{self, Library};
use crate::measure::job_sampler::JobSampler;
use crate::progress::{Progress, ProgressSink};
use crate::tasks::{self, Job, StepIo};
use crate::work_dir::store::StoreRead;
use crate::work_dir::{self, JobStore, WorkDir};
use crate::workers::{self, Binaries, StepWrite, WorkerData};
use crate::{report, resume};

use lane::LaneState;
use walk::{Begun, Steps, Tally, now_ns};

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
    /// sets it when a step fails, so the shot scan and the visual lane running alongside stop too.
    pub cancel: CancelToken,
    /// The machine-wide GPU lock file (`gpu.lock` in the app data folder).
    pub gpu_lock: PathBuf,
    /// The sign library shared by episodes (`library.redb` in the app data folder); `None` runs
    /// without one: no sign is looked up or recorded.
    pub library: Option<PathBuf>,
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
    /// The run as a whole: its start, its end and the job's peak memory over it.
    pub run: JobRun,
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
    let library = options.library.as_ref().map(Library::at);
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
        stale: resume::stale_steps(&record, &store.read()?, &work, library.as_ref()),
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
                        library: library.clone(),
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
                    let mut env = env_for(step)?;
                    if library::reads_library(step) {
                        let named = options.library.as_ref();
                        let path = named.map(|p| p.to_string_lossy().into_owned());
                        env.push((library::LOCATION_VARIABLE.into(), path.unwrap_or_default()));
                    }
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
    // The job's processes and the GPU, sampled while each step runs; the use is merged into the
    // step's measure before it is stamped.
    let sampler = JobSampler::start(std::process::id());
    let started_ns = now_ns();
    let measured = |step: StepName, record: &JobRecord| {
        sampler.open(step);
        let ran = run_step(step, record);
        let used = sampler.close(step);
        ran.map(|(mut measure, outputs)| {
            used.apply(&mut measure);
            (measure, outputs)
        })
    };

    let steps = Steps::new(
        &store,
        &record,
        library.as_ref(),
        &measured,
        progress,
        &options.cancel,
    );
    // The span of the caller's job, which the lane's step spans hang from.
    let job_span = tracing::Span::current();
    let mut tally = Tally::default();
    std::thread::scope(|scope| -> Result<()> {
        let (mut shots, mut lane) = (None, None);
        let mut lane_state = LaneState::Waiting;
        let walked = (|| -> Result<()> {
            for step in StepName::ALL {
                steps.check_cancel(step)?;
                let plan = lane::plan(step, lane_state);
                if plan.join_shots
                    && let Some(handle) = shots.take()
                {
                    steps.join_shots(handle)?;
                }
                if plan.spawn_lane {
                    lane = Some(lane::spawn(scope, &steps, &job_span));
                    lane_state = LaneState::Running;
                }
                if plan.join_lane
                    && let Some(handle) = lane.take()
                {
                    tally.merge(lane::join(handle)?);
                    lane_state = LaneState::Joined;
                }
                if !plan.run_here {
                    continue;
                }
                // Everything this step logs, its workers' and programs' lines too, is under its span.
                let step_span = tracing::info_span!("step", step = %step);
                let _in_step = step_span.enter();
                if step != StepName::ShotScan {
                    steps.run(step, &mut tally)?;
                    continue;
                }
                if let Begun::Started { fingerprint } = steps.begin(step, &mut tally)? {
                    let steps = &steps;
                    let span = step_span.clone();
                    // The scan commits its own outputs, so a later step's outputs never wait on
                    // a transaction only this thread's join would end.
                    shots = Some(scope.spawn(move || {
                        let _in_step = span.enter();
                        steps.complete(StepName::ShotScan, fingerprint)
                    }));
                }
            }
            if let Some(handle) = shots.take() {
                steps.join_shots(handle)?;
            }
            if let Some(handle) = lane.take() {
                tally.merge(lane::join(handle)?);
            }
            Ok(())
        })();
        walked.map_err(|error| {
            // The scope waits for the shot scan and the lane; stop them rather than wait them out.
            options.cancel.cancel();
            steps.job_error(error)
        })
    })?;
    let (ran, skipped) = tally.in_run_order();

    let run = JobRun {
        started_ns,
        finished_ns: now_ns(),
        peak_ram_mib: sampler.stop().peak_pss_mib,
    };
    store.put_job_run(&run)?;
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
        run,
    })
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

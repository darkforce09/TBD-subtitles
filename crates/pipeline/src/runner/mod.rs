//! The job runner: one video through every step, skipping what is still valid, the shot scan
//! alongside the GPU steps, each step's measure recorded, and the report written at the end.
//!
//! **Role:** open or create the job's work directory and record, take its lock, walk
//! `StepName::ALL`, run each stale step in process or in its worker, and record it.
//!
//! **Position:** called by the app's `process` subcommand (and later its window); uses `resume`,
//! `workers`, `tasks` and `report`.
//!
//! **Signals and state:** writes `job.json` after every finished step; emits progress events.
//!
//! **Invariants:** `job.json` always describes finished steps only, so a killed job resumes from
//! the last one; one worker loads the GPU at a time (the shot scan uses none); a step starts only
//! after every step it reads has finished.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use inference::cuda_runtime::CudaRuntime;
use job_model::StepName;
use job_model::job::{JobRecord, JobSettings, StepMeasure, StepRecord};
use job_model::outputs::ProbeDecoded;
use job_model::report::QcReport;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Placement};
use crate::progress::{Progress, ProgressSink};
use crate::tasks::{self, Job};
use crate::work_dir::{self, WorkDir};
use crate::workers::{self, Binaries};
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
    let _lock = resume::lock(&work)?;
    let video_text = video.to_string_lossy().into_owned();
    let mut record = match work_dir::read_json::<JobRecord>(&work.job_json()) {
        Ok(r) if r.video == video_text => r,
        _ => JobRecord {
            video: video_text,
            video_size: 0,
            video_modified_s: 0,
            settings: options.settings.clone(),
            models_dir: None,
            steps: Default::default(),
        },
    };
    record.video_size = meta.len();
    record.video_modified_s = modified_s;
    record.settings = options.settings.clone();
    record.models_dir = Some(options.models_dir.to_string_lossy().into_owned());
    for step in &options.rerun {
        record.steps.remove(step);
    }
    work_dir::write_json(&work.job_json(), &record)?;
    progress(Progress::JobStarted {
        video: video.clone(),
        work_dir: work.root().to_path_buf(),
        stale: resume::stale_steps(&record, &work),
    });

    let cuda_env: OnceLock<std::result::Result<Vec<(String, String)>, String>> = OnceLock::new();
    let env_for = |step: StepName| -> Result<Vec<(String, String)>> {
        if !graph::uses_gpu(step) {
            return Ok(Vec::new());
        }
        cuda_env
            .get_or_init(|| locate_cuda(&options.binaries).map_err(|e| e.to_string()))
            .clone()
            .map_err(|e| PipelineError::new(format!("step {step}"), e))
    };
    let run_step = |step: StepName, record: &JobRecord| -> Result<StepMeasure> {
        match graph::placement(step) {
            Placement::InProcess => {
                let job = Job {
                    work: work.clone(),
                    record: record.clone(),
                };
                tasks::in_process(step, &job, &|done, total| {
                    progress(Progress::StepAdvanced { step, done, total })
                })
            }
            Placement::Worker(binary) => {
                let env = env_for(step)?;
                workers::run_worker(
                    options.binaries.path(binary),
                    step,
                    &work,
                    &env,
                    progress,
                    &options.cancel,
                    &options.gpu_lock,
                )
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
                if graph::inputs(step).contains(&StepName::ShotScan)
                    && let Some(handle) = shots.take()
                {
                    let measure = join(handle)?;
                    finish(&mut record, &work, StepName::ShotScan, measure, progress)?;
                }
                if resume::is_valid(step, &record, &work) {
                    skipped.push(step);
                    progress(Progress::StepSkipped(step));
                    announce_duration(step, &work, progress);
                    continue;
                }
                record.steps.remove(&step);
                ran.push(step);
                progress(Progress::StepStarted(step));
                if step == StepName::ShotScan {
                    let snapshot = record.clone();
                    let run_step = &run_step;
                    shots = Some(scope.spawn(move || run_step(StepName::ShotScan, &snapshot)));
                    continue;
                }
                let measure = run_step(step, &record).inspect_err(|error| {
                    progress(Progress::StepFailed {
                        step,
                        message: error.to_string(),
                    })
                })?;
                finish(&mut record, &work, step, measure, progress)?;
                announce_duration(step, &work, progress);
            }
            if let Some(handle) = shots.take() {
                let measure = join(handle)?;
                finish(&mut record, &work, StepName::ShotScan, measure, progress)?;
            }
            Ok(())
        })();
        if walked.is_err() {
            // The scope waits for the shot scan; stop it rather than wait out a long scan.
            options.cancel.cancel();
        }
        walked
    })?;

    let qc = report::write(&work, &record)?;
    Ok(JobOutcome {
        work_dir: work.root().to_path_buf(),
        subtitles: stages::output::subtitle_path(&video, record.settings.output_format),
        report: work.report(),
        qc,
        ran,
        skipped,
    })
}

/// Tell the listener the video's length once the probe is there.
fn announce_duration(step: StepName, work: &WorkDir, progress: ProgressSink) {
    if step == StepName::ProbeDecode
        && let Ok(probe) = work_dir::read_json::<ProbeDecoded>(&work.probe())
    {
        progress(Progress::JobDuration(probe.probe.duration_s));
    }
}

/// The CUDA runtime's environment for GPU workers, packaged beside the binaries or in the
/// runtime folder.
fn locate_cuda(binaries: &Binaries) -> Result<Vec<(String, String)>> {
    let runtime_dir =
        inference::model_store::runtime_dir().context("cannot find the runtime folder")?;
    let runtime =
        CudaRuntime::locate(binaries.main.parent(), &runtime_dir).context("CUDA runtime")?;
    Ok(runtime.worker_env())
}

fn join(handle: std::thread::ScopedJoinHandle<'_, Result<StepMeasure>>) -> Result<StepMeasure> {
    handle
        .join()
        .map_err(|_| PipelineError::new("step shot_scan", "the scan thread panicked"))?
}

/// Record `step` as finished now and save the record.
fn finish(
    record: &mut JobRecord,
    work: &WorkDir,
    step: StepName,
    measure: StepMeasure,
    progress: ProgressSink,
) -> Result<()> {
    let fingerprint = resume::fingerprint(step, record);
    let finished_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    record.steps.insert(
        step,
        StepRecord {
            fingerprint,
            finished_ns,
            measure: measure.clone(),
        },
    );
    work_dir::write_json(&work.job_json(), record)?;
    progress(Progress::StepFinished { step, measure });
    Ok(())
}

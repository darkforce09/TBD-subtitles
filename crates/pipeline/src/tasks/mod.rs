//! The body of every step: read its inputs from the work directory, call the stage, write its
//! output. The same code runs inside the job runner (CPU steps) and inside a worker process.
//!
//! **Role:** dispatch a step to its task, time it, and, in a worker, report the step's load
//! time, processing time and peak memory to `steps/<step>.worker.json` and its progress as
//! `progress <done> <total>` lines on stdout.
//!
//! **Position:** called by `runner` (in process) and by the `worker` subcommand of both app
//! binaries; each task module calls `stages` and the backends.
//!
//! **Signals and state:** reads `job.json` and the step's inputs; writes the step's outputs.
//!
//! **Invariants:** a task writes its outputs completely or not at all (part files, renamed); a
//! Whisper step runs only in a binary built with the `crispasr` feature, and nothing else needs
//! it.

mod alignment;
mod layout;
mod llm;
mod media;
mod review;
mod sounds;
mod speech;

pub(crate) use review::corrected_lines;

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use job_model::StepName;
use job_model::job::{JobRecord, JobSettings, StepMeasure, WorkerMeasure};
use job_model::outputs::ProbeDecoded;

use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Binary, Placement};
use crate::measure::memory;
use crate::work_dir::{self, WorkDir};

/// A job as a task sees it: its folder and its record.
#[derive(Debug, Clone)]
pub struct Job {
    pub work: WorkDir,
    pub record: JobRecord,
}

impl Job {
    /// The job whose folder is `dir`, from its `job.json`.
    pub fn load(dir: &Path) -> Result<Job> {
        let work = WorkDir::new(dir);
        let record = work_dir::read_json(&work.job_json())?;
        Ok(Job { work, record })
    }

    pub fn video(&self) -> PathBuf {
        PathBuf::from(&self.record.video)
    }

    pub fn settings(&self) -> &JobSettings {
        &self.record.settings
    }

    /// The probe-and-decode result.
    pub fn probe(&self) -> Result<ProbeDecoded> {
        work_dir::read_json(&self.work.probe())
    }

    /// The folder models are read from: the one the run named, else the default.
    pub fn models(&self) -> Result<PathBuf> {
        match &self.record.models_dir {
            Some(dir) => Ok(PathBuf::from(dir)),
            None => inference::model_store::models_dir().context("cannot find the models folder"),
        }
    }

    pub fn glossary(&self) -> Vec<&str> {
        self.record
            .settings
            .glossary
            .iter()
            .map(String::as_str)
            .collect()
    }
}

/// What a task reports besides its outputs.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskReport {
    pub load_s: f64,
    pub process_s: f64,
    pub notes: BTreeMap<String, String>,
}

impl TaskReport {
    pub fn note(&mut self, key: &str, value: impl ToString) {
        self.notes.insert(key.to_string(), value.to_string());
    }
}

/// Seconds since `start`.
pub(crate) fn since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64()
}

/// Where a task reports `(done, total)`; shared by the threads of a concurrent task.
pub type StepProgress<'a> = &'a (dyn Fn(usize, usize) + Sync);

/// Run `step`'s task; `progress` hears `(done, total)`.
pub fn run(step: StepName, job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let result = match step {
        StepName::ProbeDecode => media::probe_decode(job, progress),
        StepName::ShotScan => media::shot_scan(job),
        StepName::Separation => media::separation(job, progress),
        StepName::Vad => speech::vad(job),
        StepName::AsrParakeet => speech::asr_parakeet(job, progress),
        StepName::AsrWhisper => speech::asr_whisper(job, progress),
        StepName::DiffSheet => speech::diff_sheet(job),
        StepName::SoundEvents => sounds::sound_events(job, progress),
        StepName::Adjudicate => llm::adjudicate(job, progress),
        StepName::RedecodeParakeet => speech::redecode_parakeet(job, progress),
        StepName::RedecodeWhisper => speech::redecode_whisper(job, progress),
        StepName::Readjudicate => llm::readjudicate(job, progress),
        StepName::SoundCues => sounds::sound_cues(job, progress),
        StepName::Alignment => alignment::alignment(job, progress),
        StepName::Review => review::review(job),
        StepName::Cues => layout::cues(job),
        StepName::Qc => layout::qc(job),
        StepName::Output => layout::output(job),
    };
    result.map_err(|e| PipelineError::new(format!("step {step}"), e))
}

/// Run `step` inside this process and measure it.
pub fn in_process(step: StepName, job: &Job, progress: StepProgress) -> Result<StepMeasure> {
    let reset = memory::reset_peak_ram();
    let started = Instant::now();
    let report = run(step, job, progress)?;
    Ok(StepMeasure {
        wall_s: since(started),
        load_s: Some(report.load_s),
        process_s: Some(report.process_s),
        peak_ram_mib: if reset { memory::peak_ram_mib() } else { None },
        peak_child_ram_mib: None,
        peak_vram_mib: None,
        notes: report.notes,
    })
}

/// The `worker <step> <job dir>` subcommand of the binary `binary`: run the step, print its
/// progress, and write its measure file.
pub fn worker_main(step: StepName, job_dir: &Path, binary: Binary) -> Result<()> {
    let wanted = match graph::placement(step) {
        Placement::Worker(b) => b,
        Placement::InProcess => Binary::Main,
    };
    if wanted != binary {
        let name = match wanted {
            Binary::Main => "tbd-subtitles",
            Binary::Ggml => "tbd-subtitles-ggml",
        };
        return Err(PipelineError::new(
            format!("worker {step}"),
            format!("the step runs in `{name}`"),
        ));
    }
    let job = Job::load(job_dir)?;
    let print = |done: usize, total: usize| {
        let mut out = std::io::stdout().lock();
        let _ = writeln!(out, "progress {done} {total}");
        let _ = out.flush();
    };
    let report = run(step, &job, &print)?;
    let measure = WorkerMeasure {
        load_s: report.load_s,
        process_s: report.process_s,
        peak_ram_mib: memory::peak_ram_mib().unwrap_or(0.0),
        peak_child_ram_mib: memory::peak_child_ram_mib().unwrap_or(0.0),
        notes: report.notes,
    };
    work_dir::write_json(&job.work.worker_measure(step), &measure)
}

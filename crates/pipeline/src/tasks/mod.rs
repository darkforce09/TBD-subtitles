//! The body of every step: read its inputs through its `StepIo`, call the stage, write its
//! outputs through it. The same code runs inside the job runner (CPU steps) and inside a worker
//! process.
//!
//! **Role:** dispatch a step to its task, time it, and, in a worker, send the runner the step's
//! progress, its load time, processing time and peak memory, and its end or its failure as frames
//! of the worker channel.
//!
//! **Position:** called by `runner` (in process) and by the `worker` subcommand of the three app
//! binaries; each task module calls `stages` and the backends; `io` carries every stored input
//! and output.
//!
//! **Signals and state:** a worker reads the job record and the step's documents from the `Input`
//! frames on its stdin, leaves the per-frame rows after them for the task, drains what it did not
//! read before `Done`, and installs the worker channel, which points its descriptor 1 at stderr;
//! the files a task streams (audio, crops, plates) are written in the job folder.
//!
//! **Invariants:** a task's stored outputs are committed with its step's record or not at all;
//! a file a task writes is written through a part file and renamed; a
//! Whisper step runs only in a binary built with the `crispasr` feature, and nothing else needs
//! it; a worker installs its channel before anything else, so no native library prints into the
//! frame stream, and it ends with `Measure` then `Done`, or with `Failed`.

mod alignment;
pub mod io;
mod layout;
mod llm;
mod localized;
mod media;
mod onscreen;
mod replace;
mod review;
mod rows;
mod sounds;
mod speech;
mod verify;

pub use io::StepIo;
pub(crate) use review::corrected_lines;

use std::collections::BTreeMap;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::time::Instant;

use job_model::StepName;
use job_model::job::{JobRecord, JobSettings, StepMeasure, WorkerMeasure};

use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Binary, Placement};
use crate::library::Library;
use crate::measure::memory;
use crate::work_dir::WorkDir;

/// A job as a task sees it: its folder, its record, and the sign library it reads and records
/// into, when the run uses one.
#[derive(Debug, Clone)]
pub struct Job {
    pub work: WorkDir,
    pub record: JobRecord,
    pub library: Option<Library>,
}

impl Job {
    /// The job whose folder is `dir`, with the job record `io` received and the library its
    /// runner named in `library::LOCATION_VARIABLE`.
    pub fn received(dir: &Path, io: &StepIo) -> Result<Job> {
        Ok(Job {
            work: WorkDir::new(dir),
            record: io.job_record()?,
            library: Library::from_environment(),
        })
    }

    /// The job's id, which names the signs it adds to the library.
    pub fn id(&self) -> String {
        crate::work_dir::job_id(&self.video())
    }

    pub fn video(&self) -> PathBuf {
        PathBuf::from(&self.record.video)
    }

    pub fn settings(&self) -> &JobSettings {
        &self.record.settings
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

/// Run `step`'s task on `io`; `progress` hears `(done, total)`.
pub fn run(
    step: StepName,
    job: &Job,
    io: &mut StepIo,
    progress: StepProgress,
) -> Result<TaskReport> {
    let result = match step {
        StepName::ProbeDecode => media::probe_decode(job, io, progress),
        StepName::ShotScan => media::shot_scan(job, io, progress),
        StepName::Separation => media::separation(job, io, progress),
        StepName::Vad => speech::vad(job, io, progress),
        StepName::AsrParakeet => speech::asr_parakeet(job, io, progress),
        StepName::AsrWhisper => speech::asr_whisper(job, io, progress),
        StepName::DiffSheet => speech::diff_sheet(job, io, progress),
        StepName::SoundEvents => sounds::sound_events(job, io, progress),
        StepName::Adjudicate => llm::adjudicate(job, io, progress),
        StepName::RedecodeParakeet => speech::redecode_parakeet(job, io, progress),
        StepName::RedecodeWhisper => speech::redecode_whisper(job, io, progress),
        StepName::Readjudicate => llm::readjudicate(job, io, progress),
        StepName::SoundCues => sounds::sound_cues(job, io, progress),
        StepName::Alignment => alignment::alignment(job, io, progress),
        StepName::Review => review::review(job, io, progress),
        StepName::Cues => layout::cues(job, io, progress),
        StepName::TextDetect
        | StepName::TextRead
        | StepName::TextTrack
        | StepName::TextTranslate
        | StepName::TextReview
        | StepName::TextTypeset => onscreen::run(step, job, io, progress),
        StepName::Qc => layout::qc(job, io, progress),
        StepName::TextMask | StepName::TextInpaint | StepName::TextCompose => {
            replace::run(step, job, io, progress)
        }
        StepName::TextVerify => verify::run(job, io, progress),
        StepName::Output => layout::output(job, io, progress),
        StepName::LocalizedVideo => localized::run(job, io, progress),
    };
    result.map_err(|e| PipelineError::new(format!("step {step}"), e))
}

/// Run `step` inside this process on `io` and measure it; its outputs stay in `io` for the
/// runner to commit with the step's record.
pub fn in_process(
    step: StepName,
    job: &Job,
    io: &mut StepIo,
    progress: StepProgress,
) -> Result<StepMeasure> {
    let reset = memory::reset_peak_ram();
    let started = Instant::now();
    let report = run(step, job, io, progress)?;
    Ok(StepMeasure {
        wall_s: since(started),
        load_s: Some(report.load_s),
        process_s: Some(report.process_s),
        peak_ram_mib: if reset { memory::peak_ram_mib() } else { None },
        peak_child_ram_mib: None,
        peak_vram_mib: None,
        notes: report.notes,
        ..StepMeasure::default()
    })
}

/// The `worker <step> <job dir>` subcommand of the binary `binary`: install the worker channel,
/// read the step's inputs from stdin, run the step, and send its outputs, progress and measure,
/// then its end; any error is sent as a `Failed` frame and returned.
pub fn worker_main(step: StepName, job_dir: &Path, binary: Binary) -> Result<()> {
    if std::io::stdin().is_terminal() {
        return Err(PipelineError::new(
            format!("worker {step}"),
            "a worker receives inputs on a pipe from the runner, not a terminal",
        ));
    }
    worker_main_from(step, job_dir, binary, std::io::stdin())
}

/// What a worker binary executes for `step` in `job_dir`, reading runner inputs from `stdin`.
pub fn worker_main_from(
    step: StepName,
    job_dir: &Path,
    binary: Binary,
    stdin: impl Read + Send + 'static,
) -> Result<()> {
    worker_channel::worker::install().map_err(|error| {
        PipelineError::new(
            format!("worker {step}"),
            format!("cannot open the worker channel: {error}"),
        )
    })?;
    let result = run_in_worker(step, job_dir, binary, stdin);
    if let Err(error) = &result {
        worker_channel::worker::failed(&error.to_string());
    }
    result
}

/// The worker's step, from the placement check to its `Done` frame.
fn run_in_worker(
    step: StepName,
    job_dir: &Path,
    binary: Binary,
    stdin: impl Read + Send + 'static,
) -> Result<()> {
    let context = format!("worker {step}");
    let wanted = match graph::placement(step) {
        Placement::Worker(b) => b,
        Placement::InProcess => Binary::Main,
    };
    if wanted != binary {
        let name = match wanted {
            Binary::Main => "tbd-subtitles",
            Binary::Ggml => "tbd-subtitles-ggml",
            Binary::LocalLlm => "tbd-subtitles-llm",
        };
        return Err(PipelineError::new(
            context,
            format!("the step runs in `{name}`"),
        ));
    }
    let stdin = std::io::BufReader::new(stdin);
    let mut io = StepIo::over_stdin(step, stdin)?;
    let job = Job::received(job_dir, &io)?;
    let send = |done: usize, total: usize| {
        worker_channel::worker::progress(done as u64, total as u64);
    };
    let report = run(step, &job, &mut io, &send)?;
    io.finish_inputs()?;
    let measure = WorkerMeasure {
        load_s: report.load_s,
        process_s: report.process_s,
        peak_ram_mib: memory::peak_ram_mib().unwrap_or(0.0),
        peak_child_ram_mib: memory::peak_child_ram_mib().unwrap_or(0.0),
        notes: report.notes,
    };
    let archive = rkyv::to_bytes::<rkyv::rancor::Error>(&measure).map_err(|error| {
        PipelineError::new(&context, format!("cannot archive the measure: {error}"))
    })?;
    if !worker_channel::worker::measure(&archive) || !worker_channel::worker::done() {
        return Err(PipelineError::new(
            context,
            "the runner's end of the worker channel is closed",
        ));
    }
    Ok(())
}

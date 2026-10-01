//! Worker processes: `<binary> worker <step> <job dir>`, one at a time for GPU steps, with the
//! CUDA runtime's environment, its inputs sent from the job database, its progress frames and
//! model calls forwarded, its outputs kept uncommitted, its stderr kept, and its time, peak RAM
//! and peak VRAM read back.
//!
//! **Role:** find the app binaries, start a step's worker, send its inputs, read its frames, and
//! turn what it reports into the step's measure and its uncommitted outputs.
//!
//! **Position:** called by `runner` for every step placed in a worker; uses `child_process` to
//! run it, `channel` for its inputs and outputs, `frames` to read its stdout and
//! `measure::gpu_monitor` to sample its VRAM.
//!
//! **Signals and state:** spawns the worker; streams its inputs to its stdin on a thread; reads
//! its stdout as frames; writes `logs/<step>.log`.
//!
//! **Invariants:** a worker that exits non-zero, breaks the frame protocol, or exits 0 without its
//! `Measure` and `Done` is a failed step with the end of its stderr in the error, and its outputs
//! are dropped uncommitted; a worker that breaks the protocol is killed at once; an input that
//! could not be sent fails a step that otherwise finished.

use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use child_process::Run;
use job_model::StepName;
use job_model::job::StepMeasure;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Binary};
use crate::measure::gpu_monitor;
use crate::progress::{Progress, ProgressSink};
use crate::work_dir::{self, JobStore};
use worker_channel::address::Address;

pub mod channel;
pub(crate) mod frames;
pub mod gpu_lock;

pub use channel::StepWrite;

/// Lines of a failed worker's stderr quoted in the error.
const STDERR_TAIL: usize = 12;

/// The app binaries that isolate each model runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binaries {
    pub main: PathBuf,
    pub ggml: PathBuf,
    pub local_llm: PathBuf,
}

impl Binaries {
    /// The main, ggml and local-language-model binaries beside the running executable.
    pub fn beside_current_exe() -> Result<Binaries> {
        let exe = std::env::current_exe().context("cannot find the running binary")?;
        Ok(Binaries {
            main: exe.with_file_name("tbd-subtitles"),
            ggml: exe.with_file_name("tbd-subtitles-ggml"),
            local_llm: exe.with_file_name("tbd-subtitles-llm"),
        })
    }

    pub fn path(&self, binary: Binary) -> &Path {
        match binary {
            Binary::Main => &self.main,
            Binary::Ggml => &self.ggml,
            Binary::LocalLlm => &self.local_llm,
        }
    }
}

/// What a finished worker left: its measure, and its outputs when it sent any.
#[derive(Debug)]
pub struct WorkerRun {
    pub measure: StepMeasure,
    /// The outputs it sent, uncommitted: the caller commits them with the step's record. `None`
    /// when it sent none.
    pub outputs: Option<StepWrite>,
}

/// What a worker works on: its job's store, whose work directory it runs in and which keeps its
/// outputs, and the stored values it reads, sent down its stdin.
#[derive(Debug, Clone, Copy)]
pub struct WorkerData<'a> {
    pub store: &'a Arc<JobStore>,
    pub inputs: &'a [Address],
}

/// Run `step` in a worker of `binary` on the job of `data.store`, with `env` added, and measure
/// it. A GPU step first takes the machine-wide lock at `gpu_lock`; `cancel` kills the worker, or
/// ends the wait for the lock.
pub fn run_worker(
    binary: &Path,
    step: StepName,
    data: WorkerData<'_>,
    env: &[(String, String)],
    progress: ProgressSink,
    cancel: &CancelToken,
    gpu_lock: &Path,
) -> Result<WorkerRun> {
    let context = format!("step {step}");
    if !binary.exists() {
        return Err(PipelineError::new(
            context,
            format!("{} is missing from the app's own folder", binary.display()),
        ));
    }
    let work = data.store.work();
    let mut run = Run::new(binary)
        .arg("worker")
        .arg(step.as_str())
        .arg(work.root())
        .timeout(graph::timeout(step))
        .cancel_on(cancel.flag());
    for (key, value) in env {
        run = run.env(key, value);
    }
    let rows = graph::reads_rows(step);
    if !data.inputs.is_empty() || !rows.is_empty() {
        run = run.stdin_piped();
    }
    if graph::placement(step) == graph::Placement::Worker(Binary::LocalLlm) {
        run = run
            .env_remove("ORT_DYLIB_PATH")
            .env("LD_LIBRARY_PATH", local_llm_library_path(binary)?);
    }
    let gpu = graph::uses_gpu(step);
    let _held = if gpu {
        tracing::debug!("step {step} takes the GPU lock {}", gpu_lock.display());
        let holder = gpu_lock::Holder {
            step,
            job: work.root().to_path_buf(),
        };
        Some(gpu_lock::acquire(gpu_lock, holder, cancel, &|held| {
            progress(Progress::StepMessage {
                step,
                text: gpu_lock::waiting_message(held, work.root()),
            })
        })?)
    } else {
        None
    };
    let baseline = if gpu {
        gpu_monitor::device_memory()
    } else {
        None
    };
    if let Some(b) = &baseline {
        tracing::debug!("step {step} starts with {} MiB of VRAM free", b.free_mib);
    }
    let mut worker = run.spawn().context(context.clone())?;
    let sending = worker.take_stdin().map(|stdin| {
        channel::inputs::send_inputs(data.store.clone(), data.inputs.to_vec(), rows, stdin)
    });
    let monitor = baseline
        .as_ref()
        .map(|b| gpu_monitor::Monitor::start(worker.pid(), b.used_mib));
    let mut outputs = StepWrite::new(data.store.clone());
    let read = match worker.take_stdout() {
        Some(stdout) => frames::read_frames(
            step,
            &mut BufReader::new(stdout),
            progress,
            Some(&mut outputs),
        ),
        None => Err("the worker's stdout is not a pipe".to_string()),
    };
    let report = match read {
        Ok(report) => report,
        Err(broken) => {
            // The step's outputs drop uncommitted before the worker is stopped.
            drop(outputs);
            let stopped = worker.kill_and_wait();
            let _ = sending.map(join_inputs);
            let _ = monitor.and_then(gpu_monitor::Monitor::finish);
            let stderr = match stopped {
                Err(child_process::RunError::Cancelled { .. }) => {
                    return Err(PipelineError::cancelled(context));
                }
                other => other.context(context.clone())?,
            };
            work_dir::write_text(&work.log(step), &stderr)?;
            return Err(PipelineError::new(
                context,
                format!(
                    "{broken}; the worker was stopped (log {}):\n{}",
                    work.log(step).display(),
                    tail(&stderr)
                ),
            ));
        }
    };
    let finished = worker.wait();
    let sent = sending.map_or(Ok(()), join_inputs);
    let peaks = monitor.and_then(gpu_monitor::Monitor::finish);
    let finished = match finished {
        Err(child_process::RunError::Cancelled { .. }) => {
            return Err(PipelineError::cancelled(context));
        }
        other => other.context(context.clone())?,
    };
    work_dir::write_text(&work.log(step), &finished.stderr)?;
    tracing::debug!("step {step} worker log: {}", work.log(step).display());
    let log = work.log(step);
    let reported = match report
        .verdict(finished.code)
        .and_then(|measure| sent.map(|()| measure))
    {
        Ok(measure) => measure,
        Err(why) => {
            return Err(PipelineError::new(
                context,
                format!("{why} (log {}):\n{}", log.display(), tail(&finished.stderr)),
            ));
        }
    };
    let mut notes = reported.notes;
    if let Some(p) = &peaks {
        notes.insert(
            "vram_device_delta_mib".into(),
            p.device_delta_mib.to_string(),
        );
    }
    if let Some(b) = &baseline {
        notes.insert("vram_free_before_mib".into(), b.free_mib.to_string());
    }
    let measure = StepMeasure {
        wall_s: finished.duration.as_secs_f64(),
        load_s: Some(reported.load_s),
        process_s: Some(reported.process_s),
        peak_ram_mib: Some(reported.peak_ram_mib),
        peak_child_ram_mib: (reported.peak_child_ram_mib > 0.0)
            .then_some(reported.peak_child_ram_mib),
        peak_vram_mib: peaks
            .map(|p| {
                if p.process_mib > 0 {
                    p.process_mib
                } else {
                    p.device_delta_mib
                }
            })
            .filter(|&mib| mib > 0)
            .map(|mib| mib as f64),
        notes,
        ..StepMeasure::default()
    };
    Ok(WorkerRun {
        measure,
        outputs: (outputs.received() > 0).then_some(outputs),
    })
}

/// The input thread's answer; a thread that panicked is an error too.
fn join_inputs(
    sending: std::thread::JoinHandle<std::result::Result<(), String>>,
) -> std::result::Result<(), String> {
    sending
        .join()
        .unwrap_or_else(|_| Err("the thread sending the step's inputs panicked".to_string()))
}

/// Locate only the CUDA libraries the isolated local model needs, without requiring ORT.
fn local_llm_library_path(binary: &Path) -> Result<String> {
    use inference::cuda_runtime::REQUIRED_CUDA_LIBS;
    use inference::model_store::manifest::{CUDA_FOLDER, CUDNN_FOLDER};

    let runtime = inference::model_store::runtime_dir().context("local model CUDA runtime")?;
    let candidates = binary
        .parent()
        .map(|dir| dir.join("cuda"))
        .into_iter()
        .chain(std::iter::once(runtime));
    let mut missing = Vec::new();
    for base in candidates {
        let cuda_lib = base.join(CUDA_FOLDER).join("lib");
        if let Some(library) = REQUIRED_CUDA_LIBS
            .iter()
            .find(|library| !cuda_lib.join(library).is_file())
        {
            missing.push(format!("{} lacks {library}", cuda_lib.display()));
            continue;
        }
        let mut dirs = vec![cuda_lib];
        let cudnn_lib = base.join(CUDNN_FOLDER).join("lib");
        if cudnn_lib.is_dir() {
            dirs.push(cudnn_lib);
        }
        if let Some(inherited) = std::env::var_os("LD_LIBRARY_PATH") {
            dirs.extend(
                std::env::split_paths(&inherited).filter(|path| !path.as_os_str().is_empty()),
            );
        }
        return std::env::join_paths(dirs)
            .context("local model CUDA library path")?
            .into_string()
            .map_err(|_| PipelineError::new("local model CUDA runtime", "non-UTF-8 library path"));
    }
    Err(PipelineError::new(
        "local model CUDA runtime",
        format!("no complete CUDA runtime found: {}", missing.join("; ")),
    ))
}

/// The last [`STDERR_TAIL`] lines of a worker's stderr.
fn tail(stderr: &str) -> String {
    let tail: Vec<&str> = stderr.lines().rev().take(STDERR_TAIL).collect();
    tail.into_iter().rev().collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
#[path = "tests/workers.rs"]
mod tests;

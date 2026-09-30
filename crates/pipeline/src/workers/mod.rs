//! Worker processes: `<binary> worker <step> <job dir>`, one at a time for GPU steps, with the
//! CUDA runtime's environment, its progress frames and model calls forwarded, its stderr kept,
//! and its time, peak RAM and peak VRAM read back.
//!
//! **Role:** find the app binaries, start a step's worker, read its frames, and turn what it
//! reports into the step's measure.
//!
//! **Position:** called by `runner` for every step placed in a worker; uses `child_process` to
//! run it, `frames` to read its stdout and `measure::gpu_monitor` to sample its VRAM.
//!
//! **Signals and state:** spawns the worker; reads its stdout as frames; writes `logs/<step>.log`.
//!
//! **Invariants:** a worker that exits non-zero, breaks the frame protocol, or exits 0 without its
//! `Measure` and `Done` is a failed step with the end of its stderr in the error; a worker that
//! breaks the protocol is killed at once.

use std::io::BufReader;
use std::path::{Path, PathBuf};

use child_process::Run;
use job_model::StepName;
use job_model::job::StepMeasure;

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Binary};
use crate::measure::gpu_monitor;
use crate::progress::{Progress, ProgressSink};
use crate::work_dir::{self, WorkDir};

mod frames;
pub mod gpu_lock;

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

/// Run `step` in a worker of `binary` with `env` added, and measure it. A GPU step first takes
/// the machine-wide lock at `gpu_lock`; `cancel` kills the worker, or ends the wait for the lock.
pub fn run_worker(
    binary: &Path,
    step: StepName,
    work: &WorkDir,
    env: &[(String, String)],
    progress: ProgressSink,
    cancel: &CancelToken,
    gpu_lock: &Path,
) -> Result<StepMeasure> {
    let context = format!("step {step}");
    if !binary.exists() {
        return Err(PipelineError::new(
            context,
            format!("{} is missing from the app's own folder", binary.display()),
        ));
    }
    let mut run = Run::new(binary)
        .arg("worker")
        .arg(step.as_str())
        .arg(work.root())
        .timeout(graph::timeout(step))
        .cancel_on(cancel.flag());
    for (key, value) in env {
        run = run.env(key, value);
    }
    if graph::placement(step) == graph::Placement::Worker(Binary::LocalLlm) {
        run = run
            .env_remove("ORT_DYLIB_PATH")
            .env("LD_LIBRARY_PATH", local_llm_library_path(binary)?);
    }
    let gpu = graph::uses_gpu(step);
    let _held = if gpu {
        tracing::debug!("step {step} takes the GPU lock {}", gpu_lock.display());
        Some(gpu_lock::acquire(gpu_lock, cancel, &|| {
            progress(Progress::StepMessage {
                step,
                text: "waiting for the GPU: another run of the app is using it".to_string(),
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
    let monitor = baseline
        .as_ref()
        .map(|b| gpu_monitor::Monitor::start(worker.pid(), b.used_mib));
    let read = match worker.take_stdout() {
        Some(stdout) => frames::read_frames(step, &mut BufReader::new(stdout), progress),
        None => Err("the worker's stdout is not a pipe".to_string()),
    };
    let report = match read {
        Ok(report) => report,
        Err(broken) => {
            let stopped = worker.kill_and_wait();
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
    let failed = report
        .failed
        .as_ref()
        .map(|message| format!("{message}\n"))
        .unwrap_or_default();
    let ending = if finished.code != 0 {
        format!("the worker exited {}", finished.code)
    } else {
        match (&report.measure, report.done) {
            (Some(_), true) => String::new(),
            (None, true) => "the worker exited 0 without sending its measure".to_string(),
            (Some(_), false) => "the worker exited 0 without sending its end".to_string(),
            (None, false) => {
                "the worker exited 0 without sending its measure or its end".to_string()
            }
        }
    };
    let reported = match report.measure {
        Some(measure) if ending.is_empty() => measure,
        _ => {
            return Err(PipelineError::new(
                context,
                format!(
                    "{failed}{ending} (log {}):\n{}",
                    log.display(),
                    tail(&finished.stderr)
                ),
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
    Ok(StepMeasure {
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
    })
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

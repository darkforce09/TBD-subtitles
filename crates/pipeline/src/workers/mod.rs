//! Worker processes: `<binary> worker <step> <job dir>`, one at a time for GPU steps, with the
//! CUDA runtime's environment, its progress lines forwarded, its stderr kept, and its time, peak
//! RAM and peak VRAM read back.
//!
//! **Role:** find the two app binaries, start a step's worker, and turn what it reports into the
//! step's measure.
//!
//! **Position:** called by `runner` for every step placed in a worker; uses `child_process` to
//! run it and `measure::gpu_monitor` to sample its VRAM.
//!
//! **Signals and state:** spawns the worker; writes `logs/<step>.log`; reads
//! `steps/<step>.worker.json`.
//!
//! **Invariants:** a worker that exits non-zero, or leaves no measure file, is a failed step with
//! the end of its stderr in the error; an old measure file is removed before the worker starts,
//! so it is never read as the new one's.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use child_process::Run;
use job_model::StepName;
use job_model::job::{StepMeasure, WorkerMeasure};

use crate::cancel::CancelToken;
use crate::error::{Context, PipelineError, Result};
use crate::graph::{self, Binary};
use crate::measure::gpu_monitor;
use crate::progress::{Progress, ProgressSink};
use crate::work_dir::{self, WorkDir};

pub mod gpu_lock;

/// Lines of a failed worker's stderr quoted in the error.
const STDERR_TAIL: usize = 12;

/// The two app binaries a worker runs in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binaries {
    pub main: PathBuf,
    pub ggml: PathBuf,
}

impl Binaries {
    /// `tbd-subtitles` and `tbd-subtitles-ggml` in the folder of the running binary.
    pub fn beside_current_exe() -> Result<Binaries> {
        let exe = std::env::current_exe().context("cannot find the running binary")?;
        Ok(Binaries {
            main: exe.with_file_name("tbd-subtitles"),
            ggml: exe.with_file_name("tbd-subtitles-ggml"),
        })
    }

    pub fn path(&self, binary: Binary) -> &Path {
        match binary {
            Binary::Main => &self.main,
            Binary::Ggml => &self.ggml,
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
            format!(
                "{} is missing; build it as the development environment runbook says",
                binary.display()
            ),
        ));
    }
    let measure_file = work.worker_measure(step);
    let _ = fs::remove_file(&measure_file);
    let mut run = Run::new(binary)
        .arg("worker")
        .arg(step.as_str())
        .arg(work.root())
        .timeout(graph::timeout(step))
        .cancel_on(cancel.flag());
    for (key, value) in env {
        run = run.env(key, value);
    }
    let gpu = graph::uses_gpu(step);
    let _held = if gpu {
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
    let mut worker = run.spawn().context(context.clone())?;
    let monitor = baseline
        .as_ref()
        .map(|b| gpu_monitor::Monitor::start(worker.pid(), b.used_mib));
    if let Some(stdout) = worker.take_stdout() {
        for line in BufReader::new(stdout)
            .lines()
            .map_while(std::result::Result::ok)
        {
            progress(parse_line(step, &line));
        }
    }
    let finished = worker.wait();
    let peaks = monitor.and_then(gpu_monitor::Monitor::finish);
    let finished = match finished {
        Err(child_process::RunError::Cancelled { .. }) => {
            return Err(PipelineError::cancelled(context));
        }
        other => other.context(context.clone())?,
    };
    work_dir::write_text(&work.log(step), &finished.stderr)?;
    if finished.code != 0 {
        let tail: Vec<&str> = finished.stderr.lines().rev().take(STDERR_TAIL).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        return Err(PipelineError::new(
            context,
            format!(
                "the worker exited {} (log {}):\n{}",
                finished.code,
                work.log(step).display(),
                tail.join("\n")
            ),
        ));
    }
    let reported: WorkerMeasure = work_dir::read_json(&measure_file)?;
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

/// A worker's stdout line as a progress event.
pub fn parse_line(step: StepName, line: &str) -> Progress {
    let mut parts = line.split_whitespace();
    if parts.next() == Some("progress")
        && let (Some(Ok(done)), Some(Ok(total)), None) = (
            parts.next().map(str::parse),
            parts.next().map(str::parse),
            parts.next(),
        )
    {
        return Progress::StepAdvanced { step, done, total };
    }
    Progress::StepMessage {
        step,
        text: line.to_string(),
    }
}

#[cfg(test)]
#[path = "tests/workers.rs"]
mod tests;

//! The checks that say whether this machine can run a job: the GPU and its free memory, the CUDA
//! runtime, FFmpeg and ffprobe, the `claude` CLI, and the Whisper worker binary.
//!
//! **Role:** run each check and say, in words, what it found; a check that could not run is a
//! failure with its reason, never a pass.
//!
//! **Position:** called by the application on a thread when the settings view opens and when the
//! owner asks again; uses `pipeline::measure::gpu_monitor` (NVML), `inference::cuda_runtime`,
//! `media_io::Programs` (bundled FFmpeg beside the app, else `PATH`) and `child_process` for the
//! programs' versions.
//!
//! **Signals and state:** loads NVML; runs `ffmpeg -version`, `ffmpeg -devices`,
//! `ffprobe -version` and `claude --version` with short deadlines; reads the runtime folder.
//!
//! **Invariants:** every check returns a state and a detail; nothing here changes the machine.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::time::Duration;

use child_process::Run;
use inference::cuda_runtime::CudaRuntime;
use inference::llm::claude_cli;
use media_io::Programs;
use pipeline::measure::gpu_monitor::{self, DeviceInfo};

use crate::core::background::Wake;
use crate::settings::models::machine::{Check, CheckState};

/// The name of the CUDA runtime's check, whose failure the Settings window links to the Models tab.
pub(crate) const CUDA_RUNTIME: &str = "CUDA runtime";
/// The free VRAM a GPU step needs with the desktop running, in MiB.
pub(crate) const VRAM_BUDGET_MIB: u64 = 5_632;
/// How long a version query may take.
const QUERY_DEADLINE: Duration = Duration::from_secs(20);

/// Run every check. `exe_dir` is the running binary's folder, where the Whisper worker and a
/// packaged `cuda/` folder sit; `runtime_dir` is the runtime folder.
pub(crate) fn run_all(exe_dir: Option<&Path>, runtime_dir: &Path) -> Vec<Check> {
    let programs = exe_dir.map(Programs::beside).unwrap_or_default();
    vec![
        gpu(gpu_monitor::device_info()),
        cuda_runtime(exe_dir, runtime_dir),
        ffmpeg_program("FFmpeg", &programs.ffmpeg, programs.bundled),
        pulse_output(query(&programs.ffmpeg, "-devices")),
        ffmpeg_program("ffprobe", &programs.ffprobe, programs.bundled),
        program("claude CLI", &claude_cli::resolve_program(), "--version"),
        whisper_worker(exe_dir),
    ]
}

/// Run every check on a thread; the results arrive on the returned channel, then `wake` runs.
pub(crate) fn start(
    exe_dir: Option<PathBuf>,
    runtime_dir: PathBuf,
    wake: Wake,
) -> Receiver<Vec<Check>> {
    let (send, answer) = channel();
    std::thread::spawn(move || {
        let _ = send.send(run_all(exe_dir.as_deref(), &runtime_dir));
        wake();
    });
    answer
}

/// The GPU and whether a GPU step fits in its free memory.
pub(crate) fn gpu(info: Option<DeviceInfo>) -> Check {
    let name = "GPU";
    match info {
        None => Check {
            name,
            path: None,
            state: CheckState::Failed,
            detail: "the NVIDIA driver's NVML library could not be loaded: no GPU steps can run"
                .to_string(),
        },
        Some(info) => {
            let described = format!(
                "{}, driver {}, {} of {} MiB free",
                info.name, info.driver, info.free_mib, info.total_mib
            );
            if info.free_mib >= VRAM_BUDGET_MIB {
                Check {
                    name,
                    path: None,
                    state: CheckState::Ok,
                    detail: described,
                }
            } else {
                Check {
                    name,
                    path: None,
                    state: CheckState::Warning,
                    detail: format!(
                        "{described}: a GPU step needs {VRAM_BUDGET_MIB} MiB; close programs that \
                         use the GPU"
                    ),
                }
            }
        }
    }
}

fn cuda_runtime(exe_dir: Option<&Path>, runtime_dir: &Path) -> Check {
    let name = CUDA_RUNTIME;
    match CudaRuntime::locate(exe_dir, runtime_dir) {
        Ok(runtime) => Check {
            name,
            path: Some(
                runtime
                    .cuda_root
                    .parent()
                    .unwrap_or(runtime_dir)
                    .to_path_buf(),
            ),
            state: CheckState::Ok,
            detail: "CUDA, cuDNN and ONNX Runtime found".to_string(),
        },
        Err(missing) => Check {
            name,
            path: None,
            state: CheckState::Failed,
            detail: missing.to_string(),
        },
    }
}

/// What `program argument` printed, or why it could not run.
fn query(program: &str, argument: &str) -> Result<String, String> {
    let output = Run::new(program)
        .arg("-hide_banner")
        .arg(argument)
        .timeout(QUERY_DEADLINE)
        .output();
    match output {
        Ok(out) if out.code == 0 => Ok(out.stdout),
        Ok(out) => Err(format!("{program} exited {}", out.code)),
        Err(error) => Err(error.to_string()),
    }
}

fn program(name: &'static str, program: &str, argument: &str) -> Check {
    // `claude` knows no `-hide_banner`; FFmpeg and ffprobe print a banner without it.
    let output = if name == "claude CLI" {
        Run::new(program)
            .arg(argument)
            .timeout(QUERY_DEADLINE)
            .output()
            .map_err(|e| e.to_string())
            .and_then(|out| {
                (out.code == 0)
                    .then_some(out.stdout)
                    .ok_or_else(|| format!("{program} exited {}", out.code))
            })
    } else {
        query(program, argument)
    };
    version_check(name, output)
}

/// FFmpeg or ffprobe's version check, noting in its detail whether the copy run was the one
/// bundled beside the app or one found on `PATH`.
fn ffmpeg_program(name: &'static str, program: &str, bundled: bool) -> Check {
    let mut check = version_check(name, query(program, "-version"));
    if check.state == CheckState::Ok {
        check.detail = with_source(bundled, &check.detail);
    }
    check
}

/// Prefix a passing check's detail with which copy of the program answered.
fn with_source(bundled: bool, detail: &str) -> String {
    let source = if bundled { "bundled" } else { "on PATH" };
    format!("{source}: {detail}")
}

/// A program's check from its version output.
pub(crate) fn version_check(name: &'static str, output: Result<String, String>) -> Check {
    match output {
        Ok(text) => Check {
            name,
            path: None,
            state: CheckState::Ok,
            detail: text
                .lines()
                .find(|l| !l.trim().is_empty())
                .unwrap_or("found")
                .trim()
                .to_string(),
        },
        Err(reason) => Check {
            name,
            path: None,
            state: CheckState::Failed,
            detail: reason,
        },
    }
}

/// Whether FFmpeg can play a clip's sound: its PulseAudio output device, which PipeWire serves.
pub(crate) fn pulse_output(devices: Result<String, String>) -> Check {
    let name = "Clip playback";
    match devices {
        Ok(text)
            if text
                .lines()
                .any(|l| l.split_whitespace().nth(1) == Some("pulse")) =>
        {
            Check {
                name,
                path: None,
                state: CheckState::Ok,
                detail: "FFmpeg plays sound through its pulse output".to_string(),
            }
        }
        Ok(_) => Check {
            name,
            path: None,
            state: CheckState::Warning,
            detail: "this FFmpeg has no pulse output: clips play without sound".to_string(),
        },
        Err(reason) => Check {
            name,
            path: None,
            state: CheckState::Warning,
            detail: format!("FFmpeg's devices could not be listed: {reason}"),
        },
    }
}

fn whisper_worker(exe_dir: Option<&Path>) -> Check {
    let name = "Whisper worker";
    match exe_dir.map(|dir| dir.join("tbd-subtitles-ggml")) {
        Some(path) if path.is_file() => Check {
            name,
            path: None,
            state: CheckState::Ok,
            detail: format!("{}", path.display()),
        },
        Some(path) => Check {
            name,
            path: None,
            state: CheckState::Failed,
            detail: format!(
                "{} is missing; build it as the development environment runbook says",
                path.display()
            ),
        },
        None => Check {
            name,
            path: None,
            state: CheckState::Failed,
            detail: "the running binary's folder is unknown".to_string(),
        },
    }
}

#[cfg(test)]
#[path = "tests/system_check.rs"]
mod tests;

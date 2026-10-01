//! Full-resolution text screening benchmark.
//!
//! **Role:** on one clip of a video, measure how fast frames reach a Rust reader through four
//! FFmpeg routes and how fast Rust turns YUV 4:2:0 into RGB; how fast the PP-OCRv5 mobile
//! detector screens the clip's sample frames at full resolution through the stock predictor and a
//! padded path, at several batch sizes and with two workers, against the 640 × 360 proxy
//! baseline; and what the server detector costs per still. Each section prints as a Markdown
//! table under one host line.
//! **Position:** the `detect-bench` command of the validation tool. It runs on the host (CUDA,
//! NVML, the bundled FFmpeg) and re-executes itself once with the CUDA runtime's library path and
//! `ORT_DYLIB_PATH`, as the pipeline starts a GPU worker.
//! **Signals and state:** the sample frames in memory for the detector sections; one sampler per
//! measured row.
//! **Invariants:** the source video is only read; a configuration that fails prints its error in
//! its row and the bench goes on.

mod decode;
mod detector;
mod runs;
mod table;
mod usage;
mod yuv;

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use inference::cuda_runtime::CudaRuntime;
use inference::model_store;
use media_io::Programs;
use pipeline::measure::gpu_monitor;

use decode::Clip;
use runs::Limits;

/// Set in the re-executed process, whose environment already holds the CUDA runtime.
const PREPARED: &str = "TBD_DETECT_BENCH_RUNTIME";
/// The production screening proxy's width.
const PROXY_WIDTH: u32 = 640;
/// How many stills the server detector section times.
const CONFIRM_STILLS: usize = 20;

/// The `detect-bench` command's arguments.
#[derive(clap::Args, Debug, Clone, PartialEq)]
pub struct Options {
    /// The video to measure; only read.
    pub video: PathBuf,
    /// Where the clip starts, in seconds.
    #[arg(long, default_value_t = 600.0)]
    pub start: f64,
    /// How long the clip runs, in seconds.
    #[arg(long, default_value_t = 120.0)]
    pub duration: f64,
    /// The folder holding `ffmpeg` and `ffprobe`; the pair beside this binary, else `PATH`.
    #[arg(long)]
    pub ffmpeg_dir: Option<PathBuf>,
    /// The model store holding `pp-ocrv5/`; the app's own when not given.
    #[arg(long)]
    pub models_dir: Option<PathBuf>,
    /// The folder holding the CUDA, cuDNN and ONNX Runtime folders; `cuda/` beside this binary,
    /// else the app's runtime folder.
    #[arg(long)]
    pub runtime_dir: Option<PathBuf>,
    /// Each ONNX Runtime session's CUDA arena limit, in MiB.
    #[arg(long, default_value_t = 3072)]
    pub memory_limit_mib: u64,
    /// The arena limit a failed configuration is tried once more at, in MiB.
    #[arg(long, default_value_t = 4608)]
    pub raised_limit_mib: u64,
}

impl Options {
    /// The clip, when its start and duration are usable.
    fn clip(&self) -> Result<Clip> {
        anyhow::ensure!(
            self.start.is_finite()
                && self.start >= 0.0
                && self.duration.is_finite()
                && self.duration > 0.0,
            "the clip needs a start of at least 0 and a positive duration"
        );
        anyhow::ensure!(
            self.memory_limit_mib > 0,
            "the arena limit must be positive"
        );
        Ok(Clip {
            start_s: self.start,
            duration_s: self.duration,
        })
    }
}

/// Run the whole benchmark and print its tables.
pub fn run(options: &Options) -> Result<()> {
    let clip = options.clip()?;
    prepare_runtime(options)?;
    let programs = programs(options.ffmpeg_dir.as_deref())?;
    let models = match &options.models_dir {
        Some(dir) => dir.clone(),
        None => model_store::models_dir()?,
    };
    let probe = media_io::probe::probe(&programs, &options.video)?;
    let video = probe.video.context("the file has no video stream")?;
    anyhow::ensure!(video.frame_rate_den > 0, "the video reports no frame rate");
    let fps = f64::from(video.frame_rate_num) / f64::from(video.frame_rate_den);
    let size = (video.width, video.height);
    let step = ((fps / 2.0).round() as u32).max(1);
    let proxy = (
        PROXY_WIDTH,
        even(PROXY_WIDTH * video.height / video.width.max(1)),
    );
    let baseline_mib = gpu_monitor::device_memory().map_or(0, |m| m.used_mib);

    println!("# detect-bench: {}\n", file_name(&options.video));
    println!(
        "Clip {:.0}–{:.0} s of {}×{} {} at {fps:.3} fps; sample step {step}; proxy {}×{}.  ",
        clip.start_s,
        clip.start_s + clip.duration_s,
        size.0,
        size.1,
        video.codec,
        proxy.0,
        proxy.1
    );
    println!("{}  ", host_line());
    println!(
        "FFmpeg {}; arena limit {} MiB per session, raised {} MiB on failure; ONNX Runtime {}.\n",
        programs.ffmpeg,
        options.memory_limit_mib,
        options.raised_limit_mib,
        std::env::var("ORT_DYLIB_PATH").unwrap_or_default()
    );

    decode::section(&programs, &options.video, clip, size);
    let samples = decode::samples(&programs, &options.video, clip, size, step, false)?;
    let proxies = decode::samples(&programs, &options.video, clip, proxy, step, true)?;
    let limits = Limits {
        memory_limit_mib: options.memory_limit_mib,
        raised_limit_mib: options.raised_limit_mib,
        baseline_mib,
    };
    let ocr = models.join("pp-ocrv5");
    runs::screening(&ocr.join("det_mobile.onnx"), &samples, &proxies, &limits);
    drop(proxies);
    let stills = &samples[..samples.len().min(CONFIRM_STILLS)];
    runs::confirmation(&ocr.join("det.onnx"), stills, &limits);
    Ok(())
}

/// Re-execute this process with the CUDA runtime's environment unless it already has it.
fn prepare_runtime(options: &Options) -> Result<()> {
    if std::env::var_os(PREPARED).is_some() {
        return Ok(());
    }
    let exe = std::env::current_exe().context("locate this binary")?;
    let runtime_dir = match &options.runtime_dir {
        Some(dir) => dir.clone(),
        None => model_store::runtime_dir()?,
    };
    let runtime = CudaRuntime::locate(exe.parent(), &runtime_dir)?;
    let error = std::process::Command::new(&exe)
        .args(std::env::args_os().skip(1))
        .envs(runtime.worker_env())
        .env(PREPARED, "1")
        .exec();
    Err(error).context("re-execute with the CUDA runtime")
}

/// The FFmpeg pair in `dir`, or the one beside this binary (else on `PATH`) without it.
fn programs(dir: Option<&Path>) -> Result<Programs> {
    let Some(dir) = dir else {
        return Ok(Programs::beside_current_exe());
    };
    let (ffmpeg, ffprobe) = (dir.join("ffmpeg"), dir.join("ffprobe"));
    anyhow::ensure!(
        ffmpeg.is_file() && ffprobe.is_file(),
        "{} lacks ffmpeg or ffprobe",
        dir.display()
    );
    Ok(Programs {
        ffmpeg: ffmpeg.to_string_lossy().into_owned(),
        ffprobe: ffprobe.to_string_lossy().into_owned(),
        bundled: true,
    })
}

/// The GPU, its driver and free memory, and the CPU's thread count.
fn host_line() -> String {
    let threads = std::thread::available_parallelism().map_or(0, |n| n.get());
    match gpu_monitor::device_info() {
        Some(gpu) => format!(
            "Host: {}, driver {}, {} MiB free of {} MiB before the bench; {threads} CPU threads.",
            gpu.name, gpu.driver, gpu.free_mib, gpu.total_mib
        ),
        None => format!("Host: no NVML device found; {threads} CPU threads."),
    }
}

/// `value` rounded down to an even number.
fn even(value: u32) -> u32 {
    value & !1
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "tests/options.rs"]
mod tests;

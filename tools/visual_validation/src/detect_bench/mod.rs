//! Full-resolution text screening benchmark.
//!
//! **Role:** on one clip of a video, measure how fast frames reach a Rust reader through four
//! FFmpeg routes and how fast Rust turns YUV 4:2:0 into RGB; how fast the PP-OCRv5 mobile
//! detector screens the clip's sample frames at full resolution through the stock predictor and a
//! padded path, at several batch sizes and with two workers, against the 640 × 360 proxy
//! baseline; what the server detector costs per still; and how the production detector pool
//! runs by pool and batch, with one session or two, with each search mode and on TensorRT at
//! FP16 and FP32, for screening and confirmation. It can write sample frames with the pool's
//! and the proxy's boxes drawn. Each section prints as a Markdown table under one host line.
//! **Position:** the `detect-bench` command of the validation tool. It runs on the host (CUDA,
//! NVML, the bundled FFmpeg) and re-executes itself once with the CUDA runtime's library path and
//! `ORT_DYLIB_PATH`, as the pipeline starts a GPU worker.
//! **Signals and state:** the sample frames in memory for the detector sections; one sampler per
//! measured row; TensorRT engines in the cache folder; box images in the frames folder.
//! **Invariants:** the source video is only read; a configuration that fails prints its error in
//! its row and the bench goes on; files are written only under the TensorRT cache folder and the
//! frames folder.

mod decode;
mod detector;
mod overlay;
mod pool_runs;
mod runs;
mod table;
mod usage;

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use inference::cuda_runtime::CudaRuntime;
use inference::model_store;
use inference::ocr::detector_pool::{DEFAULT_VRAM_CAP_MIB, TENSORRT_FOLDER};
use inference::ocr::pool::ScreenShape;
use media_io::Programs;
use media_io::yuv::Coefficients;
use pipeline::measure::gpu_monitor;

use decode::Clip;
use pool_runs::{Sections, Setup};
use runs::Limits;

/// Set in the re-executed process, whose environment already holds the CUDA runtime.
const PREPARED: &str = "TBD_DETECT_BENCH_RUNTIME";
/// Set in the child process that runs only sections 2 and 3.
const OAR_OCR_CHILD: &str = "TBD_DETECT_BENCH_OAR_OCR";
/// The production screening proxy's width.
const PROXY_WIDTH: u32 = 640;
/// How many stills the server detector section times.
const CONFIRM_STILLS: usize = 20;
/// How many stills the pool's confirmation section takes, the warm ones included.
const POOL_CONFIRM_STILLS: usize = 40;

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
    /// Each oar-ocr session's CUDA arena limit, in MiB.
    #[arg(long, default_value_t = 3072)]
    pub memory_limit_mib: u64,
    /// The arena limit a failed oar-ocr configuration is tried once more at, in MiB.
    #[arg(long, default_value_t = 4608)]
    pub raised_limit_mib: u64,
    /// Skip section 1: the decode routes and the conversion timing.
    #[arg(long)]
    pub no_decode: bool,
    /// Skip sections 2 and 3: the oar-ocr stock and padded paths and the server detector.
    #[arg(long)]
    pub no_oar_ocr: bool,
    /// Skip section 4: the CUDA pool × batch sweep.
    #[arg(long)]
    pub no_sweep: bool,
    /// Skip section 5: one pool session against two.
    #[arg(long)]
    pub no_sessions: bool,
    /// Skip section 6: the fast against the deterministic search.
    #[arg(long)]
    pub no_search: bool,
    /// Skip every TensorRT row: section 7 and the TensorRT rows of section 8.
    #[arg(long)]
    pub no_tensorrt: bool,
    /// Skip section 8: confirmation through the pool.
    #[arg(long)]
    pub no_pool_confirm: bool,
    /// The batches the CUDA and TensorRT sweeps run, comma-separated.
    #[arg(long, value_delimiter = ',', default_values_t = [2usize, 4, 8])]
    pub sweep_batches: Vec<usize>,
    /// The session memory pools the sweeps run each batch at, in MiB, comma-separated.
    #[arg(long, value_delimiter = ',', default_values_t = [1536usize, 2048, 2560, 3072])]
    pub sweep_pools_mib: Vec<usize>,
    /// The memory pools confirmation runs each engine at, in MiB, comma-separated.
    #[arg(long, value_delimiter = ',', default_values_t = [1536usize, 2048, 2560, 3072])]
    pub confirm_pools_mib: Vec<usize>,
    /// The batch sections 5 to 8 run at; the sweep's fastest, else the production default, when
    /// neither this nor `--shape-pool-mib` is given.
    #[arg(long)]
    pub shape_batch: Option<usize>,
    /// The memory pool sections 5 to 8 run at, in MiB.
    #[arg(long)]
    pub shape_pool_mib: Option<usize>,
    /// Where TensorRT engines are built and reused: a first run measures the build, a later one
    /// the cached engine. A bench folder under the system's temporary folder when not given.
    #[arg(long)]
    pub trt_cache: Option<PathBuf>,
    /// The GPU worker's memory cap TensorRT's workspace is sized within, in MiB.
    #[arg(long, default_value_t = DEFAULT_VRAM_CAP_MIB)]
    pub vram_cap_mib: usize,
    /// Write full-resolution sample frames here as PNGs, with the pool's CUDA and TensorRT FP16
    /// boxes and the proxy's boxes scaled up, each drawn in its own colour.
    #[arg(long)]
    pub frames_dir: Option<PathBuf>,
    /// How many sample frames, spread over the clip, `--frames-dir` receives.
    #[arg(long, default_value_t = 6)]
    pub frames_count: usize,
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

    /// The shape sections 5 to 8 run at, when one is given; a missing half is the production
    /// default's.
    fn shape(&self) -> Result<Option<ScreenShape>> {
        if self.shape_batch.is_none() && self.shape_pool_mib.is_none() {
            return Ok(None);
        }
        let shape = ScreenShape {
            batch: self.shape_batch.unwrap_or(ScreenShape::INITIAL.batch),
            pool_mib: self.shape_pool_mib.unwrap_or(ScreenShape::INITIAL.pool_mib),
        };
        anyhow::ensure!(
            shape.batch > 0 && shape.pool_mib > 0,
            "the shape needs a batch and a pool above zero"
        );
        Ok(Some(shape))
    }

    /// The pool sections to run.
    fn sections(&self) -> Result<Sections> {
        let sections = Sections {
            grid: pool_runs::grid(&self.sweep_batches, &self.sweep_pools_mib),
            shape: self.shape()?,
            cuda_sweep: !self.no_sweep,
            sessions: !self.no_sessions,
            search: !self.no_search,
            tensorrt: !self.no_tensorrt,
            confirm: !self.no_pool_confirm,
            confirm_pools_mib: self
                .confirm_pools_mib
                .iter()
                .copied()
                .filter(|&pool| pool > 0)
                .collect(),
            reference: self.frames_dir.is_some(),
        };
        anyhow::ensure!(
            !(sections.cuda_sweep || sections.tensorrt) || !sections.grid.is_empty(),
            "the sweep needs at least one batch and one pool size above zero"
        );
        anyhow::ensure!(
            !sections.confirm || !sections.confirm_pools_mib.is_empty(),
            "confirmation needs at least one pool size above zero"
        );
        anyhow::ensure!(
            self.frames_dir.is_none() || self.frames_count > 0,
            "--frames-dir needs a frame count above zero"
        );
        Ok(sections)
    }

    /// The TensorRT cache folder: the given one, else the bench's own under the temporary folder.
    fn trt_cache(&self) -> PathBuf {
        self.trt_cache.clone().unwrap_or_else(|| {
            std::env::temp_dir()
                .join("tbd-subtitles-detect-bench")
                .join(TENSORRT_FOLDER)
        })
    }
}

/// Run the whole benchmark and print its tables.
pub fn run(options: &Options) -> Result<()> {
    let clip = options.clip()?;
    let sections = options.sections()?;
    prepare_runtime(options)?;
    // Sections 2 and 3 give each session its own CUDA provider, which the OCR worker's
    // environment refuses; they run in a child process of their own.
    let oar_ocr_child = std::env::var_os(OAR_OCR_CHILD).is_some();
    if !oar_ocr_child {
        inference::ocr::strict_cuda_environment().map_err(|error| anyhow::anyhow!("{error}"))?;
    }
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
    let device = gpu_monitor::device_info();
    let setup = Setup {
        models_root: models.clone(),
        identity: pool_runs::identity(device.as_ref()),
        cache_dir: options.trt_cache(),
        vram_cap_mib: options.vram_cap_mib,
        frame: size,
        baseline_mib,
    };
    let ocr = models.join("pp-ocrv5");
    if oar_ocr_child {
        let proxies = decode::samples(&programs, &options.video, clip, proxy, step, true)?;
        let source = Source {
            programs: &programs,
            video: &options.video,
            clip,
            size,
            step,
        };
        let limits = Limits {
            memory_limit_mib: options.memory_limit_mib,
            raised_limit_mib: options.raised_limit_mib,
            baseline_mib,
        };
        return oar_ocr_sections(&source, &ocr, proxies, &limits);
    }

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
        "FFmpeg {}; arena limit {} MiB per oar-ocr session, raised {} MiB on failure; ONNX \
         Runtime {}.  ",
        programs.ffmpeg,
        options.memory_limit_mib,
        options.raised_limit_mib,
        std::env::var("ORT_DYLIB_PATH").unwrap_or_default()
    );
    println!(
        "TensorRT {}; engines cached in {}; worker cap {} MiB.\n",
        setup.identity.tensorrt_version,
        setup.cache_dir.display(),
        setup.vram_cap_mib
    );

    if !options.no_decode {
        decode::section(&programs, &options.video, clip, size);
    }
    if !options.no_oar_ocr {
        run_oar_ocr_child()?;
    }
    let mut proxy_boxes = None;
    if options.frames_dir.is_some() {
        let proxies = decode::samples(&programs, &options.video, clip, proxy, step, true)?;
        let model = ocr.join("det_mobile.onnx");
        proxy_boxes = Some(overlay::proxy_boxes(&model, &proxies, options.frames_count));
    }

    let pool_wanted = sections.cuda_sweep
        || sections.sessions
        || sections.search
        || sections.tensorrt
        || sections.confirm
        || sections.reference;
    if !pool_wanted {
        return Ok(());
    }
    let colour = Coefficients::of(&video);
    let mut frames = Vec::new();
    decode::yuv_samples(&programs, &options.video, clip, size, step, |bytes| {
        frames.push(pool_runs::padded_frame(bytes, size, &colour).map_err(anyhow::Error::msg)?);
        Ok(())
    })?;
    let stills = &frames[..frames.len().min(POOL_CONFIRM_STILLS)];
    let pool = pool_runs::run_sections(&setup, &frames, stills, &sections);
    if let Some(dir) = &options.frames_dir {
        let destination = overlay::Destination {
            dir,
            start_s: clip.start_s,
            step_s: f64::from(step) / fps,
        };
        let proxy_boxes = proxy_boxes.unwrap_or_else(|| Err("no proxy boxes".into()));
        overlay::section(&destination, &frames, &pool, &proxy_boxes);
    }
    Ok(())
}

/// Where the sample frames come from: every `step`-th frame of `clip`, at `size`.
struct Source<'a> {
    programs: &'a Programs,
    video: &'a Path,
    clip: Clip,
    size: (u32, u32),
    step: u32,
}

/// Sections 2 and 3: the oar-ocr paths over rgb24 samples held in memory, and the server
/// detector on stills.
fn oar_ocr_sections(
    source: &Source<'_>,
    ocr: &Path,
    proxies: Vec<image::RgbImage>,
    limits: &Limits,
) -> Result<()> {
    let Source {
        programs,
        video,
        clip,
        size,
        step,
    } = *source;
    let samples = decode::samples(programs, video, clip, size, step, false)?;
    runs::screening(&ocr.join("det_mobile.onnx"), &samples, &proxies, limits);
    drop(proxies);
    let stills = &samples[..samples.len().min(CONFIRM_STILLS)];
    runs::confirmation(&ocr.join("det.onnx"), stills, limits);
    Ok(())
}

/// Run sections 2 and 3 in a child of this process, which prints their tables in place.
fn run_oar_ocr_child() -> Result<()> {
    let exe = std::env::current_exe().context("locate this binary")?;
    let status = std::process::Command::new(&exe)
        .args(std::env::args_os().skip(1))
        .env(OAR_OCR_CHILD, "1")
        .status()
        .context("start the oar-ocr sections")?;
    if !status.success() {
        println!("Sections 2 and 3 stopped: {status}.\n");
    }
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

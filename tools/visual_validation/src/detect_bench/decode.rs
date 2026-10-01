//! How fast a clip's frames reach a Rust reader, and the sample frames the detector sections use.
//!
//! **Role:** decode every frame of the clip through four FFmpeg routes (rgb24 on the default
//! pipe, rgb24 and yuv420p on an enlarged pipe, NVDEC downloaded as nv12 on an enlarged pipe)
//! into one reused buffer, measuring wall time, CPU cores and GPU and NVDEC use; time the Rust
//! YUV 4:2:0 conversion of one frame; and hold every sample-step frame of the clip in memory at
//! full resolution or as a proxy.
//! **Position:** the decode section of `detect-bench` and the frame source of its detector
//! sections; runs FFmpeg as a child through `child_process`, and enlarges its pipe through
//! `media_io::video_frames::pipe`.
//! **Signals and state:** one FFmpeg child and its stdout pipe per route.
//! **Invariants:** the source is only read; the pipe is enlarged on the read end before reading,
//! never past the system's `pipe-max-size`; a route that fails reports its error in its row.

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::ChildStdout;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use child_process::{Run, Running};
use image::RgbImage;
use media_io::Programs;
use media_io::video_frames::pipe;

use super::table::{Table, optional};
use super::usage::Sampler;
use media_io::yuv::{self, Coefficients, Matrix, Range, Yuv420};

/// The part of the video measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    pub start_s: f64,
    pub duration_s: f64,
}

/// One way of getting frames out of FFmpeg.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Rgb24,
    Rgb24Wide,
    Yuv420pWide,
    NvdecNv12Wide,
}

impl Route {
    pub const ALL: [Route; 4] = [
        Route::Rgb24,
        Route::Rgb24Wide,
        Route::Yuv420pWide,
        Route::NvdecNv12Wide,
    ];

    fn label(self) -> &'static str {
        match self {
            Route::Rgb24 => "rgb24, default pipe",
            Route::Rgb24Wide => "rgb24, enlarged pipe",
            Route::Yuv420pWide => "yuv420p, enlarged pipe",
            Route::NvdecNv12Wide => "NVDEC → nv12, enlarged pipe",
        }
    }

    fn wide(self) -> bool {
        self != Route::Rgb24
    }

    /// The byte size of one frame of `size`.
    pub fn frame_bytes(self, (width, height): (u32, u32)) -> usize {
        let pixels = width as usize * height as usize;
        match self {
            Route::Rgb24 | Route::Rgb24Wide => pixels * 3,
            Route::Yuv420pWide | Route::NvdecNv12Wide => pixels * 3 / 2,
        }
    }

    /// FFmpeg's arguments for every frame of `clip` at `size`, raw on stdout.
    pub fn args(self, video: &Path, clip: Clip, size: (u32, u32)) -> Vec<String> {
        let (input, filter, format): (&[&str], String, &str) = match self {
            Route::Rgb24 | Route::Rgb24Wide => {
                (&[], format!("scale={}:{}", size.0, size.1), "rgb24")
            }
            Route::Yuv420pWide => (&[], "null".into(), "yuv420p"),
            Route::NvdecNv12Wide => (
                &["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"],
                "hwdownload,format=nv12".into(),
                "nv12",
            ),
        };
        ffmpeg_args(input, video, clip, &filter, format)
    }
}

/// FFmpeg's arguments: `input` options, `clip` of `video`'s first video stream through `filter`,
/// as raw `format` pixels on stdout with no frame-rate conversion.
pub fn ffmpeg_args(
    input: &[&str],
    video: &Path,
    clip: Clip,
    filter: &str,
    format: &str,
) -> Vec<String> {
    let mut args: Vec<String> = ["-nostdin", "-hide_banner", "-v", "error", "-noautorotate"]
        .iter()
        .chain(input)
        .map(|arg| arg.to_string())
        .collect();
    args.extend(["-ss".into(), clip.start_s.to_string(), "-i".into()]);
    args.push(video.to_string_lossy().into_owned());
    args.extend(["-t".into(), clip.duration_s.to_string()]);
    args.extend(
        [
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-fps_mode",
            "passthrough",
            "-vf",
        ]
        .map(String::from),
    );
    args.push(filter.to_string());
    args.extend(["-pix_fmt", format, "-f", "rawvideo", "pipe:1"].map(String::from));
    args
}

/// Enlarge `pipe` through `media_io` when `wide`; the pipe's size afterwards.
fn size_pipe(pipe: &ChildStdout, wide: bool) -> Result<usize> {
    let size = if wide {
        pipe::enlarge(pipe)
    } else {
        pipe::capacity(pipe)
    };
    Ok(size?)
}

/// Start FFmpeg with `args` and take its stdout.
fn spawn(programs: &Programs, args: Vec<String>) -> Result<(Running, ChildStdout)> {
    let mut decoder = Run::new(&programs.ffmpeg)
        .args(args)
        .timeout(Duration::from_secs(3600))
        .spawn()
        .context("start FFmpeg")?;
    let pipe = decoder.take_stdout().context("FFmpeg has no stdout")?;
    Ok((decoder, pipe))
}

/// Fill `buffer` from `pipe`: `true` when full, `false` at a clean end before any byte.
fn read_frame(pipe: &mut ChildStdout, buffer: &mut [u8]) -> Result<bool> {
    let mut filled = 0;
    while filled < buffer.len() {
        match pipe.read(&mut buffer[filled..]) {
            Ok(0) if filled == 0 => return Ok(false),
            Ok(0) => anyhow::bail!("FFmpeg ended inside a frame"),
            Ok(n) => filled += n,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(error.into()),
        }
    }
    Ok(true)
}

/// Reap `decoder`, failing on a non-zero exit.
fn reap(decoder: Running) -> Result<()> {
    let finished = decoder.wait()?;
    anyhow::ensure!(
        finished.code == 0,
        "FFmpeg exited {}: {}",
        finished.code,
        finished.stderr.trim()
    );
    Ok(())
}

/// What one route measured, and its first frame.
struct Decoded {
    pipe_bytes: usize,
    frames: u64,
    first: Vec<u8>,
}

/// Decode every frame of `clip` through `route` into one reused buffer.
fn decode(
    programs: &Programs,
    video: &Path,
    clip: Clip,
    size: (u32, u32),
    route: Route,
) -> Result<Decoded> {
    let (decoder, mut pipe) = spawn(programs, route.args(video, clip, size))?;
    let pipe_bytes = size_pipe(&pipe, route.wide())?;
    let mut buffer = vec![0u8; route.frame_bytes(size)];
    let mut first = Vec::new();
    let mut frames = 0u64;
    while read_frame(&mut pipe, &mut buffer)? {
        if frames == 0 {
            first = buffer.clone();
        }
        frames += 1;
    }
    drop(pipe);
    reap(decoder)?;
    Ok(Decoded {
        pipe_bytes,
        frames,
        first,
    })
}

/// Measure every route over `clip` and the Rust conversion of its first YUV frames; print both
/// tables.
pub fn section(programs: &Programs, video: &Path, clip: Clip, size: (u32, u32)) {
    println!("## 1. Decode to a Rust reader\n");
    let mut table = Table::new(&[
        "Route",
        "Pipe KiB",
        "Frames",
        "Wall s",
        "Frames/s",
        "CPU cores",
        "GPU %",
        "NVDEC %",
        "Note",
    ]);
    let mut planar = None;
    let mut nv12 = None;
    for route in Route::ALL {
        let sampler = Sampler::start();
        let decoded = decode(programs, video, clip, size, route);
        let usage = sampler.finish();
        match decoded {
            Ok(decoded) => {
                table.row(vec![
                    route.label().into(),
                    (decoded.pipe_bytes / 1024).to_string(),
                    decoded.frames.to_string(),
                    format!("{:.2}", usage.wall_s),
                    format!("{:.0}", decoded.frames as f64 / usage.wall_s),
                    optional(usage.cpu_cores, 1),
                    optional(usage.gpu_pct, 0),
                    optional(usage.decoder_pct, 0),
                    String::new(),
                ]);
                match route {
                    Route::Yuv420pWide => planar = Some(decoded.first),
                    Route::NvdecNv12Wide => nv12 = Some(decoded.first),
                    _ => {}
                }
            }
            Err(error) => table.row(vec![
                route.label().into(),
                String::new(),
                String::new(),
                format!("{:.2}", usage.wall_s),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                format!("error: {error:#}"),
            ]),
        }
    }
    println!("{}", table.render());
    conversion(size, planar.as_deref(), nv12.as_deref());
}

/// Time the Rust conversion of one decoded frame per layout, on one thread and across rayon.
fn conversion(size: (u32, u32), planar: Option<&[u8]>, nv12: Option<&[u8]>) {
    const RUNS: u32 = 30;
    let (width, height) = (size.0 as usize, size.1 as usize);
    let mut table = Table::new(&[
        "Conversion of one frame",
        "ms/frame",
        "Same bytes as scalar",
    ]);
    let frames = [
        (
            "yuv420p",
            planar.and_then(|f| Yuv420::planar(f, width, height)),
        ),
        ("nv12", nv12.and_then(|f| Yuv420::nv12(f, width, height))),
    ];
    for (layout, frame) in frames {
        let Some(frame) = frame else {
            table.row(vec![format!("{layout}: no frame decoded")]);
            continue;
        };
        let colour = Coefficients::new(Matrix::Bt709, Range::Limited);
        let mut serial = vec![0u8; frame.rgb_len()];
        let mut parallel = vec![0u8; frame.rgb_len()];
        let time = |convert: &dyn Fn(&mut [u8]) -> bool, out: &mut [u8]| {
            convert(out);
            let started = Instant::now();
            for _ in 0..RUNS {
                convert(out);
            }
            started.elapsed().as_secs_f64() * 1000.0 / f64::from(RUNS)
        };
        let scalar_ms = time(&|out| yuv::to_rgb(&frame, &colour, out), &mut serial);
        let parallel_ms = time(
            &|out| yuv::to_rgb_parallel(&frame, &colour, out),
            &mut parallel,
        );
        table.row(vec![
            format!("{layout}, scalar integer BT.709"),
            format!("{scalar_ms:.2}"),
            "—".into(),
        ]);
        table.row(vec![
            format!("{layout}, rayon rows"),
            format!("{parallel_ms:.2}"),
            if serial == parallel { "yes" } else { "NO" }.into(),
        ]);
    }
    println!("{}", table.render());
}

/// Every `step`-th frame of `clip`, decoded at `size` as rgb24; with `proxy`, FFmpeg skips the
/// in-loop deblocking filter as the production screening proxy does.
pub fn samples(
    programs: &Programs,
    video: &Path,
    clip: Clip,
    size: (u32, u32),
    step: u32,
    proxy: bool,
) -> Result<Vec<RgbImage>> {
    let input: &[&str] = if proxy {
        &["-skip_loop_filter", "all"]
    } else {
        &[]
    };
    let filter = format!("select=not(mod(n\\,{step})),scale={}:{}", size.0, size.1);
    let (decoder, mut pipe) = spawn(programs, ffmpeg_args(input, video, clip, &filter, "rgb24"))?;
    size_pipe(&pipe, true)?;
    let mut frames = Vec::new();
    loop {
        let mut buffer = vec![0u8; Route::Rgb24.frame_bytes(size)];
        if !read_frame(&mut pipe, &mut buffer)? {
            break;
        }
        frames.push(RgbImage::from_raw(size.0, size.1, buffer).context("frame buffer size")?);
    }
    drop(pipe);
    reap(decoder)?;
    anyhow::ensure!(!frames.is_empty(), "the clip has no frames");
    Ok(frames)
}

#[cfg(test)]
#[path = "tests/decode.rs"]
mod tests;

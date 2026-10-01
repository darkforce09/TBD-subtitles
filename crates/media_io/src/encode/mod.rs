//! Encoding raw frames into a video beside the source, with the source's audio and chapters.
//!
//! **Role:** choose the video encoder this FFmpeg can run, build the command line that reads raw
//! frames on stdin and muxes them with the source's audio, chapters and metadata into Matroska,
//! and drive that FFmpeg through [`EncoderProcess`]; [`segments`] instead re-encodes only the
//! changed pieces of an H.264 source and joins them to its copied packets.
//! **Position:** media output for the localized-video step; called by the localize stage in
//! `stages` with frames from `video_frames::FrameStream::open_native`.
//! **Signals and state:** `available_encoder` runs two bounded FFmpeg probes (the encoder list and
//! a one-frame NVENC test); the encoder process holds its stdin pipe and a count of frames
//! written.
//! **Invariants:** the source is only read; the output holds the new video stream, every source
//! audio stream copied unchanged, the source chapters and metadata, and no subtitle or data
//! stream. Colour tags are written only when the probe knows them. A variable-frame-rate
//! timeline is detected before encoding, since raw frames on a pipe carry one constant rate.

mod process;
pub mod segments;

pub use process::EncoderProcess;

use std::path::{Path, PathBuf};
use std::time::Duration;

use child_process::Run;
use job_model::outputs::VideoStream;

use crate::video_frames::PixelFormat;
use crate::{MediaError, Programs};

/// The deadline for listing encoders and for the one-frame test encode.
const PROBE_DEADLINE: Duration = Duration::from_secs(60);
/// How far a frame may stray from its constant-rate position, as a share of one frame.
const RATE_TOLERANCE: f64 = 0.01;
/// The timestamp rounding a constant-rate timeline may carry: a millisecond container clock.
const TIMESTAMP_ROUNDING_S: f64 = 0.001;

/// A video encoder FFmpeg runs for the localized video.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoder {
    /// NVIDIA's hardware HEVC encoder, at constant quality.
    HevcNvenc,
    /// The x264 software H.264 encoder, at constant rate factor.
    Libx264,
}

impl Encoder {
    /// FFmpeg's name for the encoder, as `-c:v` takes it.
    pub fn name(self) -> &'static str {
        match self {
            Encoder::HevcNvenc => "hevc_nvenc",
            Encoder::Libx264 => "libx264",
        }
    }
}

/// A video's colour description, each part only when the probe knows it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VideoColour {
    /// Colour primaries, as `-color_primaries` takes them (`bt709`).
    pub primaries: Option<String>,
    /// Transfer characteristics, as `-color_trc` takes them (`bt709`).
    pub transfer: Option<String>,
    /// Matrix coefficients, as `-colorspace` takes them (`bt709`).
    pub matrix: Option<String>,
    /// Sample range, as `-color_range` takes it (`tv` or `pc`).
    pub range: Option<String>,
}

impl VideoColour {
    /// The colour tags the probe found on `stream`.
    pub fn of(stream: &VideoStream) -> VideoColour {
        VideoColour {
            primaries: stream.color_primaries.clone(),
            transfer: stream.color_transfer.clone(),
            matrix: stream.color_space.clone(),
            range: stream.color_range.clone(),
        }
    }
}

/// Everything one encode needs: the raw frames' layout and rate, the source to take audio,
/// chapters and metadata from, the output file and the encoder.
#[derive(Debug, Clone, PartialEq)]
pub struct EncodeSpec {
    pub width: u32,
    pub height: u32,
    /// The constant frame rate as a fraction (numerator, denominator), such as (24000, 1001).
    pub frame_rate: (u32, u32),
    /// The layout of the raw frames written to the encoder.
    pub pixel_format: PixelFormat,
    /// When the first frame presents, in seconds on the source's container clock.
    pub video_offset_s: f64,
    pub colour: VideoColour,
    /// The source video's bit rate in bits per second; caps the encoder's peak rate so the
    /// localized video stays near the source's size.
    pub source_bit_rate: Option<u64>,
    pub source: PathBuf,
    pub output: PathBuf,
    pub encoder: Encoder,
}

/// The encoder for the localized video: NVENC HEVC when FFmpeg lists it and a one-frame test
/// encode succeeds (a driver and a free session exist), else libx264; an error when neither is
/// available.
pub fn available_encoder(programs: &Programs) -> Result<Encoder, MediaError> {
    let listing = Run::new(&programs.ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .timeout(PROBE_DEADLINE)
        .output()?;
    if listing.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffmpeg.clone(),
            code: listing.code,
            stderr: listing.stderr,
        });
    }
    if lists_encoder(&listing.stdout, Encoder::HevcNvenc.name()) && nvenc_runs(programs) {
        return Ok(Encoder::HevcNvenc);
    }
    if lists_encoder(&listing.stdout, Encoder::Libx264.name()) {
        return Ok(Encoder::Libx264);
    }
    Err(MediaError::Parse(
        "FFmpeg has neither the hevc_nvenc nor the libx264 encoder".into(),
    ))
}

/// Whether one frame encodes with NVENC; any failure to run or a non-zero exit is a no.
fn nvenc_runs(programs: &Programs) -> bool {
    Run::new(&programs.ffmpeg)
        .args(nvenc_test_args())
        .timeout(PROBE_DEADLINE)
        .output()
        .is_ok_and(|output| output.code == 0)
}

/// FFmpeg's arguments for a one-frame NVENC HEVC encode to nowhere.
fn nvenc_test_args() -> Vec<String> {
    [
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-f",
        "lavfi",
        "-i",
        "color=size=256x256:rate=24",
        "-frames:v",
        "1",
        "-c:v",
        "hevc_nvenc",
        "-f",
        "null",
        "-",
    ]
    .map(String::from)
    .to_vec()
}

/// Whether `ffmpeg -encoders` output lists an encoder named exactly `name`: each entry is a
/// capability column followed by the name.
fn lists_encoder(listing: &str, name: &str) -> bool {
    listing
        .lines()
        .any(|line| line.split_whitespace().nth(1) == Some(name))
}

/// FFmpeg's arguments for an encode: raw frames on stdin at a constant rate, offset to the
/// source's video start, muxed with every audio stream of the source copied, its chapters and
/// metadata, and no subtitle or data stream, into Matroska. An existing output is overwritten.
pub fn encode_args(spec: &EncodeSpec) -> Vec<String> {
    let mut args = strings(&["-hide_banner", "-v", "error", "-y", "-f", "rawvideo"]);
    args.extend(["-pix_fmt".into(), spec.pixel_format.name().into()]);
    args.extend(["-s".into(), format!("{}x{}", spec.width, spec.height)]);
    let (num, den) = spec.frame_rate;
    args.extend(["-framerate".into(), format!("{num}/{den}")]);
    args.extend(["-itsoffset".into(), format!("{:.6}", spec.video_offset_s)]);
    args.extend(strings(&["-i", "pipe:0", "-i"]));
    args.push(path(&spec.source));
    args.extend(strings(&[
        "-map",
        "0:v:0",
        "-map",
        "1:a?",
        "-map_chapters",
        "1",
        "-map_metadata",
        "1",
        "-sn",
        "-dn",
        "-c:a",
        "copy",
    ]));
    args.extend(codec_args(
        spec.encoder,
        spec.pixel_format,
        spec.source_bit_rate,
    ));
    args.extend(colour_args(&spec.colour));
    args.extend(strings(&[
        "-max_muxing_queue_size",
        "4096",
        "-f",
        "matroska",
    ]));
    args.push(path(&spec.output));
    args
}

/// Peak rate over the source's bit rate: HEVC keeps the source's quality at about its rate, H.264
/// needs a little more to survive a second generation.
const HEVC_PEAK_SHARE: f64 = 1.25;
const H264_PEAK_SHARE: f64 = 1.5;
/// NVENC's preset for the whole-video HEVC encode: the encode-bench measured `p4` at the same
/// quality and size as `p6` and `p7`, over twice as fast.
pub const HEVC_NVENC_PRESET: &str = "p4";

/// The video codec's arguments: constant quality capped near the source's bit rate when it is
/// known, 10-bit profiles for 10-bit frames, and 4:2:0 output for rgb24 frames.
fn codec_args(encoder: Encoder, format: PixelFormat, source_bit_rate: Option<u64>) -> Vec<String> {
    let ten_bit = format.is_high_bit_depth();
    let mut args = match encoder {
        Encoder::HevcNvenc => strings(&[
            "-c:v",
            "hevc_nvenc",
            "-preset",
            HEVC_NVENC_PRESET,
            "-tune",
            "hq",
            "-rc",
            "vbr",
            "-cq",
            "19",
            "-b:v",
            "0",
            "-profile:v",
            if ten_bit { "main10" } else { "main" },
        ]),
        Encoder::Libx264 => strings(&["-c:v", "libx264", "-preset", "slow", "-crf", "16"]),
    };
    if encoder == Encoder::Libx264 && ten_bit {
        args.extend(strings(&["-profile:v", "high10"]));
    }
    if let Some(rate) = source_bit_rate {
        let share = match encoder {
            Encoder::HevcNvenc => HEVC_PEAK_SHARE,
            Encoder::Libx264 => H264_PEAK_SHARE,
        };
        let peak = (rate as f64 * share).round() as u64;
        args.extend([
            "-maxrate".into(),
            peak.to_string(),
            "-bufsize".into(),
            (2 * peak).to_string(),
        ]);
    }
    if format == PixelFormat::Rgb24 {
        args.extend(strings(&["-pix_fmt", "yuv420p"]));
    }
    args
}

/// The colour tags the probe knows, in the order primaries, transfer, matrix, range.
fn colour_args(colour: &VideoColour) -> Vec<String> {
    [
        ("-color_primaries", &colour.primaries),
        ("-color_trc", &colour.transfer),
        ("-colorspace", &colour.matrix),
        ("-color_range", &colour.range),
    ]
    .into_iter()
    .filter_map(|(flag, value)| {
        value
            .as_ref()
            .map(|value| [flag.to_string(), value.clone()])
    })
    .flatten()
    .collect()
}

/// Whether raw frames at `fps` keep every frame's time: each frame starts where a constant rate
/// puts it, `index / fps` after the first, to within 1 % of a frame plus a millisecond. The
/// millisecond is the rounding of containers that store times in milliseconds, as Matroska
/// does, where 23.976 fps frames last 41 or 42 ms. Checking positions rather than single
/// durations also catches a slow drift; the last frame's own duration is free, and a timeline of
/// one frame or none is constant.
pub fn is_constant_frame_rate(timeline: &[(f64, f64)], fps: f64) -> bool {
    if !fps.is_finite() || fps <= 0.0 {
        return false;
    }
    let nominal = 1.0 / fps;
    let tolerance = nominal * RATE_TOLERANCE + TIMESTAMP_ROUNDING_S;
    let Some(&(first, _)) = timeline.first() else {
        return true;
    };
    timeline
        .iter()
        .enumerate()
        .all(|(index, &(start, _))| (start - first - index as f64 * nominal).abs() <= tolerance)
}

fn strings(parts: &[&str]) -> Vec<String> {
    parts.iter().map(|part| part.to_string()).collect()
}

fn path(file: &Path) -> String {
    file.to_string_lossy().into_owned()
}

#[cfg(test)]
#[path = "tests/encode.rs"]
mod tests;

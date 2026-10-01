//! The encode benchmark's FFmpeg command lines and the reading of FFmpeg's PSNR line.
//!
//! **Role:** describe the clip's decode into raw frames on a pipe, the whole-video HEVC encode of
//! those frames at a chosen NVENC preset, and the PSNR comparison of an encoded clip with the
//! source; read the average PSNR from FFmpeg's log.
//! **Position:** used by `encode_bench::measure`; the segment rows take their command line from
//! `media_io::encode::segments::segment_args` instead.
//! **Signals and state:** none; pure functions over their arguments.
//! **Invariants:** the decode and the comparison read the same clip of the first video stream
//! with no frame-rate conversion; the HEVC encode uses the production NVENC settings apart from
//! the preset, with no audio.

use std::path::Path;

use media_io::video_frames::PixelFormat;

/// The x264 presets the segment rows time, fastest first.
pub const X264_PRESETS: [&str; 8] = [
    "ultrafast",
    "veryfast",
    "faster",
    "fast",
    "medium",
    "slow",
    "slower",
    "veryslow",
];
/// The NVENC presets, fastest first.
pub const NVENC_PRESETS: [&str; 7] = ["p1", "p2", "p3", "p4", "p5", "p6", "p7"];
/// The production whole-video encode's peak rate over the source's, for NVENC HEVC.
pub const HEVC_PEAK_SHARE: f64 = 1.25;

/// The part of the video measured.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    pub start_s: f64,
    pub duration_s: f64,
}

/// The raw frames a whole-video encode reads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RawInput {
    pub size: (u32, u32),
    pub frame_rate: (u32, u32),
    pub format: PixelFormat,
}

/// FFmpeg's arguments for every frame of `clip` of `video`'s first video stream, raw in
/// `format` on stdout with no frame-rate conversion.
pub fn decode_args(video: &Path, clip: Clip, format: PixelFormat) -> Vec<String> {
    let mut args = strings(&["-nostdin", "-hide_banner", "-v", "error", "-noautorotate"]);
    args.extend(clip_input(video, clip));
    args.extend(strings(&[
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        format.name(),
        "-f",
        "rawvideo",
        "pipe:1",
    ]));
    args
}

/// FFmpeg's arguments for the whole-video HEVC encode of raw `input` frames on stdin at NVENC
/// `preset`, video alone into Matroska at `output`; the peak rate capped at
/// [`HEVC_PEAK_SHARE`] times `source_bit_rate` when it is known.
pub fn hevc_args(
    input: RawInput,
    preset: &str,
    source_bit_rate: Option<u64>,
    output: &Path,
) -> Vec<String> {
    let mut args = strings(&["-hide_banner", "-v", "error", "-y", "-f", "rawvideo"]);
    args.extend(["-pix_fmt".into(), input.format.name().into()]);
    args.extend(["-s".into(), format!("{}x{}", input.size.0, input.size.1)]);
    let (num, den) = input.frame_rate;
    args.extend(["-framerate".into(), format!("{num}/{den}")]);
    args.extend(strings(&[
        "-i",
        "pipe:0",
        "-map",
        "0:v:0",
        "-c:v",
        "hevc_nvenc",
    ]));
    args.extend(["-preset".into(), preset.into()]);
    args.extend(strings(&[
        "-tune", "hq", "-rc", "vbr", "-cq", "19", "-b:v", "0",
    ]));
    let profile = if input.format.is_high_bit_depth() {
        "main10"
    } else {
        "main"
    };
    args.extend(["-profile:v".into(), profile.into()]);
    if let Some(rate) = source_bit_rate {
        let peak = (rate as f64 * HEVC_PEAK_SHARE).round() as u64;
        args.extend([
            "-maxrate".into(),
            peak.to_string(),
            "-bufsize".into(),
            (2 * peak).to_string(),
        ]);
    }
    args.extend(strings(&["-f", "matroska"]));
    args.push(output.to_string_lossy().into_owned());
    args
}

/// FFmpeg's arguments to compare `encoded` with `clip` of `video`, frame by frame: each picture
/// is stamped with its index, since Matroska rounds times to the millisecond and pairing by time
/// would match neighbouring frames; the `psnr` filter logs the mean over every frame.
pub fn psnr_args(video: &Path, clip: Clip, encoded: &Path) -> Vec<String> {
    let mut args = strings(&["-nostdin", "-hide_banner", "-nostats", "-v", "info"]);
    args.extend(clip_input(video, clip));
    args.push("-i".into());
    args.push(encoded.to_string_lossy().into_owned());
    args.extend(strings(&[
        "-lavfi",
        "[0:v:0]settb=1,setpts=N[source];[1:v:0]settb=1,setpts=N[encoded];\
         [encoded][source]psnr",
        "-f",
        "null",
        "-",
    ]));
    args
}

/// The average PSNR in dB from FFmpeg's `psnr` log line, `inf` for identical frames.
pub fn parse_psnr(log: &str) -> Option<f64> {
    log.lines()
        .filter(|line| line.contains("PSNR"))
        .flat_map(str::split_whitespace)
        .find_map(|part| part.strip_prefix("average:"))
        .and_then(|value| value.parse().ok())
}

/// `-ss <start> -t <duration> -i <video>`: an accurate seek to the clip.
fn clip_input(video: &Path, clip: Clip) -> Vec<String> {
    vec![
        "-ss".into(),
        format!("{:.3}", clip.start_s),
        "-t".into(),
        format!("{:.3}", clip.duration_s),
        "-i".into(),
        video.to_string_lossy().into_owned(),
    ]
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

#[cfg(test)]
#[path = "tests/args.rs"]
mod tests;

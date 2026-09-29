//! One decoded frame at a presentation time.
//!
//! **Role:** decode the single frame of a video that presents at an origin-relative time, scaled
//! to a size, as rgb24 bytes, for a caller that needs one picture rather than a stream.
//! **Position:** beside `FrameStream` in `video_frames`, taking times from its timeline.
//! **Signals and state:** one bounded FFmpeg run whose stdout is read to at most one byte past a
//! frame.
//! **Invariants:** the seek lands half a frame before `time_s`. FFmpeg's accurate seek decodes
//! from the keyframe before the seek point and drops every frame that presents earlier than it,
//! so the first frame left is the one at `time_s`, even when printed timestamps are rounded, and
//! the seek clamps at the start. `-ss` counts from the container's start time, the origin the
//! timeline uses. Anything but exactly one full frame is an error; the source is only read.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use child_process::Run;

use super::{frame_bytes, parse_io, valid_rate};
use crate::{MediaError, Programs};

/// One frame at `time_s` (origin-relative) scaled to `size`, as rgb24 bytes of exactly w*h*3.
pub fn still(
    programs: &Programs,
    video: &Path,
    time_s: f64,
    fps: f64,
    size: (u32, u32),
) -> Result<Vec<u8>, MediaError> {
    let bytes = frame_bytes(size)?;
    if !time_s.is_finite() || time_s < 0.0 || !valid_rate(fps) {
        return Err(MediaError::Parse("invalid still frame timing".into()));
    }
    let mut running = Run::new(&programs.ffmpeg)
        .args(still_args(video, time_s, fps, size))
        .timeout(Duration::from_secs(120))
        .spawn()?;
    let mut rgb = Vec::with_capacity(bytes + 1);
    running
        .take_stdout()
        .ok_or_else(|| MediaError::Parse("no video pipe".into()))?
        .take(bytes as u64 + 1)
        .read_to_end(&mut rgb)
        .map_err(parse_io)?;
    let finished = running.wait()?;
    if finished.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffmpeg.clone(),
            code: finished.code,
            stderr: finished.stderr,
        });
    }
    if rgb.len() != bytes {
        return Err(MediaError::Parse("still frame is incomplete".into()));
    }
    Ok(rgb)
}

/// FFmpeg's arguments for the first frame that presents no earlier than half a frame before
/// `time_s`, scaled to `size`, as raw rgb24 on stdout.
fn still_args(video: &Path, time_s: f64, fps: f64, size: (u32, u32)) -> Vec<String> {
    let seek = (time_s - 0.5 / fps).max(0.0);
    let mut args = Vec::from(
        [
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-seek_timestamp",
            "0",
            "-ss",
        ]
        .map(String::from),
    );
    args.push(format!("{seek:.6}"));
    args.extend(["-noautorotate", "-i"].map(String::from));
    args.push(video.to_string_lossy().into_owned());
    args.extend(["-map", "0:v:0", "-an", "-sn", "-dn", "-vf"].map(String::from));
    args.push(format!("scale={}:{}", size.0, size.1));
    args.extend(
        [
            "-frames:v",
            "1",
            "-fps_mode",
            "passthrough",
            "-pix_fmt",
            "rgb24",
            "-f",
            "rawvideo",
            "pipe:1",
        ]
        .map(String::from),
    );
    args
}

#[cfg(test)]
#[path = "tests/still.rs"]
mod tests;

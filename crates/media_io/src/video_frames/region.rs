//! A run of consecutive frames cropped to one region at full resolution.
//!
//! **Role:** decode `count` consecutive source frames starting at a presentation time, cropped to
//! a rectangle of source pixels, as rgb24 bytes, for the stages that measure and replace visible
//! writing in one region of the picture.
//! **Position:** beside `FrameStream` and `still` in `video_frames`, taking times from the same
//! timeline.
//! **Signals and state:** one FFmpeg run whose stdout carries `count` crops; the stream holds one
//! crop at a time and the count read.
//! **Invariants:** the seek is `still`'s: half a frame before `first_time_s`, so the first crop
//! is the frame at that time. The frame converts to rgb24 at full size before the crop, so a crop
//! at odd coordinates is exact and its pixels equal the same rectangle of a full-size rgb24
//! frame. Anything but exactly `count` full crops is an error; a caller that stops early drops
//! the stream, which kills FFmpeg; the source is only read.

use std::path::Path;
use std::process::ChildStdout;
use std::time::Duration;

use child_process::{Run, Running};

use super::{fill, frame_bytes, has_more, valid_rate};
use crate::{MediaError, Programs};

/// The deadline for starting a run: opening the file and decoding from the keyframe before it.
const BASE_DEADLINE_S: u64 = 120;
/// The extra deadline per requested frame; full-resolution decoding runs far faster.
const DEADLINE_PER_FRAME_S: f64 = 0.25;

/// `count` consecutive frames from `first_time_s`, each cropped to one rectangle, as rgb24.
pub struct RegionStream {
    decoder: Running,
    pixels: ChildStdout,
    bytes: usize,
    count: u64,
    read: u64,
    program: String,
}

impl RegionStream {
    /// Start decoding `count` frames from the frame at `first_time_s` (origin-relative), each
    /// cropped to `crop` = (x, y, width, height) of source pixels. `fps` sets the half-frame seek
    /// margin; a caller that knows the gap to the previous frame passes its inverse. The crop
    /// must lie inside the frame: FFmpeg moves a crop that overflows an edge back inside, and
    /// fails only one larger than the frame.
    pub fn open(
        programs: &Programs,
        video: &Path,
        crop: (u32, u32, u32, u32),
        first_time_s: f64,
        fps: f64,
        count: u64,
    ) -> Result<Self, MediaError> {
        let bytes = frame_bytes((crop.2, crop.3))?;
        if !first_time_s.is_finite() || first_time_s < 0.0 || !valid_rate(fps) || count == 0 {
            return Err(MediaError::Parse("invalid region frame request".into()));
        }
        let mut decoder = Run::new(&programs.ffmpeg)
            .args(region_args(video, crop, first_time_s, fps, count))
            .timeout(deadline(count))
            .spawn()?;
        let pixels = decoder
            .take_stdout()
            .ok_or_else(|| MediaError::Parse("no video pipe".into()))?;
        Ok(Self {
            decoder,
            pixels,
            bytes,
            count,
            read: 0,
            program: programs.ffmpeg.clone(),
        })
    }

    /// The next crop as rgb24 bytes of exactly width*height*3, or `None` once `count` crops were
    /// read or FFmpeg ended; a partial crop is an error.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, MediaError> {
        if self.read == self.count {
            return Ok(None);
        }
        let mut rgb = vec![0; self.bytes];
        match fill(&mut self.pixels, &mut rgb)? {
            0 => Ok(None),
            n if n == self.bytes => {
                self.read += 1;
                Ok(Some(rgb))
            }
            _ => Err(MediaError::Parse("region frame is incomplete".into())),
        }
    }

    /// Reap FFmpeg: an error unless it exited cleanly after exactly `count` crops were read.
    pub fn finish(self) -> Result<(), MediaError> {
        let RegionStream {
            decoder,
            mut pixels,
            count,
            read,
            program,
            ..
        } = self;
        let surplus = if read == count {
            has_more(&mut pixels)
        } else {
            Ok(false)
        };
        drop(pixels);
        let waited = decoder.wait();
        if surplus? {
            return Err(MediaError::Parse(format!(
                "region decode passed {count} frames"
            )));
        }
        let finished = waited?;
        if finished.code != 0 {
            return Err(MediaError::Exit {
                program,
                code: finished.code,
                stderr: finished.stderr,
            });
        }
        if read != count {
            return Err(MediaError::Parse(format!(
                "region decode ended after {read} of {count} frames"
            )));
        }
        Ok(())
    }
}

/// The run's deadline: a fixed start-up allowance plus a share per frame.
fn deadline(count: u64) -> Duration {
    let per_frame = (count as f64 * DEADLINE_PER_FRAME_S).min(24.0 * 3600.0);
    Duration::from_secs(BASE_DEADLINE_S) + Duration::from_secs_f64(per_frame)
}

/// FFmpeg's arguments for `count` frames from the first that presents no earlier than half a
/// frame before `first_time_s`, converted to rgb24 at full size, then cropped, on stdout.
fn region_args(
    video: &Path,
    crop: (u32, u32, u32, u32),
    first_time_s: f64,
    fps: f64,
    count: u64,
) -> Vec<String> {
    let seek = (first_time_s - 0.5 / fps).max(0.0);
    let (x, y, width, height) = crop;
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
    args.push(format!(
        "scale=iw:ih,format=rgb24,crop={width}:{height}:{x}:{y}"
    ));
    args.push("-frames:v".into());
    args.push(count.to_string());
    args.extend(
        [
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
#[path = "tests/region.rs"]
mod tests;

//! Bounded video decoding with source presentation timestamps.
//!
//! **Role:** stream a video's frames from FFmpeg as raw pixels (rgb24 scaled to a size, or every
//! frame at its native size in a chosen pixel format for the encoder), each paired with its
//! presentation interval from the container's packet table, without retaining the video; `still`
//! decodes a single frame of the same timeline and `region` a run of cropped frames.
//! **Position:** media input for text detection, tracking, replacement, the localized-video
//! encoder and validation tools.
//! **Signals and state:** the packet timeline (`packets.rs`, two bounded ffprobe runs) before
//! decoding; then one streaming FFmpeg decoder and its stdout pipe. The stream holds the
//! timeline, one frame at a time and the count of frames handed out.
//! **Invariants:** no frame-rate conversion. The decoder yields exactly one frame per timeline
//! entry: a missing, partial or surplus frame is an error, never a shifted timestamp. Every
//! timestamp uses the common container origin, preserving the video offset relative to audio.
//! Dropping the stream kills the decoder; the source is only read.

mod native;
mod packets;
pub mod region;
pub mod still;

pub use native::PixelFormat;
pub use packets::timeline;

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::ChildStdout;
use std::time::Duration;

use crate::{MediaError, Programs};
use child_process::{Run, Running};

#[cfg(test)]
use packets::{
    MAX_LINE_BYTES, MAX_PACKETS, PACKET_TIME, container_origin, discarded, frame_interval,
    parse_origin, parse_packets, parse_timing, presentation_timeline,
};

/// How the decoder is asked to work: exact output, or a fast proxy for screening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decode {
    /// Every decoding step as encoded: the frames are the source pictures.
    Exact,
    /// The in-loop deblocking filter is skipped where the codec allows it, as H.264 and HEVC do:
    /// faster, with block edges that build up until the next keyframe. For screening only.
    Proxy,
}

/// One decoded frame: its presentation index, origin-relative interval and pixels, which are
/// rgb24 from [`FrameStream::open`] and the chosen [`PixelFormat`] from
/// [`FrameStream::open_native`].
pub struct VideoFrame {
    pub index: u64,
    pub time_s: f64,
    pub end_s: f64,
    pub rgb: Vec<u8>,
}

/// Every frame of a video's first video stream, decoded in presentation order.
pub struct FrameStream {
    decoder: Running,
    pixels: ChildStdout,
    bytes: usize,
    timeline: Vec<(f64, f64)>,
    read: usize,
    program: String,
}

impl FrameStream {
    /// Read the video's presentation timeline, then start decoding its frames at `size`.
    ///
    /// `start_s` is the origin when the container reports none; `1 / fps` ends the final frame
    /// when its packet reports no duration.
    pub fn open(
        programs: &Programs,
        video: &Path,
        size: (u32, u32),
        start_s: f64,
        fps: f64,
        decode: Decode,
    ) -> Result<Self, MediaError> {
        let bytes = frame_bytes(size)?;
        let timeline = timeline(programs, video, start_s, fps)?;
        Self::start(programs, decoder_args(video, size, decode), bytes, timeline)
    }

    /// Start the decoder with `args` over a timeline read beforehand; `bytes` is one frame.
    fn start(
        programs: &Programs,
        args: Vec<String>,
        bytes: usize,
        timeline: Vec<(f64, f64)>,
    ) -> Result<Self, MediaError> {
        let mut decoder = Run::new(&programs.ffmpeg)
            .args(args)
            .timeout(Duration::from_secs(24 * 3600))
            .spawn()?;
        let pixels = decoder
            .take_stdout()
            .ok_or_else(|| MediaError::Parse("no video pipe".into()))?;
        Ok(Self {
            decoder,
            pixels,
            bytes,
            timeline,
            read: 0,
            program: programs.ffmpeg.clone(),
        })
    }

    /// Origin-relative (time_s, end_s) of every frame, by presentation index, known before decoding.
    pub fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    /// The next frame, or `None` once the decoder has ended after one frame per timeline entry.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, MediaError> {
        let frame = read_frame(&mut self.pixels, self.bytes, &self.timeline, self.read)?;
        if frame.is_some() {
            self.read += 1;
        }
        Ok(frame)
    }

    /// Reap the decoder: an error unless it exited cleanly after exactly one frame per timeline
    /// entry was read. A caller that stops early drops the stream instead.
    pub fn finish(self) -> Result<(), MediaError> {
        let FrameStream {
            decoder,
            mut pixels,
            timeline,
            read,
            program,
            ..
        } = self;
        let complete = read == timeline.len();
        let surplus = if complete {
            has_more(&mut pixels)
        } else {
            Ok(false)
        };
        drop(pixels);
        let waited = decoder.wait();
        if surplus? {
            return Err(disagreement());
        }
        let finished = waited?;
        if finished.code != 0 {
            return Err(MediaError::Exit {
                program,
                code: finished.code,
                stderr: finished.stderr,
            });
        }
        if !complete {
            return Err(disagreement());
        }
        Ok(())
    }
}

/// The byte size of one rgb24 frame at `size`, which must be non-zero and at most 256 MiB.
fn frame_bytes(size: (u32, u32)) -> Result<usize, MediaError> {
    (size.0 as usize)
        .checked_mul(size.1 as usize)
        .and_then(|v| v.checked_mul(3))
        .filter(|v| *v > 0 && *v <= 256 * 1024 * 1024)
        .ok_or_else(|| MediaError::Parse("invalid or excessive video dimensions".into()))
}

/// Whether `fps` is a positive frame rate whose frame duration is finite.
fn valid_rate(fps: f64) -> bool {
    fps.is_finite() && fps > 0.0 && (1.0 / fps).is_finite()
}

/// FFmpeg's arguments for every frame of the first video stream, scaled to `size`, as raw rgb24
/// on stdout with no frame-rate conversion.
fn decoder_args(video: &Path, size: (u32, u32), decode: Decode) -> Vec<String> {
    stream_args(
        video,
        Some(format!("scale={}:{}", size.0, size.1)),
        PixelFormat::Rgb24,
        decode,
    )
}

/// FFmpeg's arguments for every frame of the first video stream through `filter`, if any, as raw
/// `format` pixels on stdout with no frame-rate conversion.
fn stream_args(
    video: &Path,
    filter: Option<String>,
    format: PixelFormat,
    decode: Decode,
) -> Vec<String> {
    let proxy: &[&str] = match decode {
        Decode::Exact => &[],
        Decode::Proxy => &["-skip_loop_filter", "all"],
    };
    let mut args: Vec<String> = ["-nostdin", "-hide_banner", "-v", "error", "-noautorotate"]
        .iter()
        .chain(proxy)
        .chain(&["-i"])
        .map(|arg| arg.to_string())
        .collect();
    args.push(video.to_string_lossy().into_owned());
    args.extend(
        [
            "-map",
            "0:v:0",
            "-an",
            "-sn",
            "-dn",
            "-fps_mode",
            "passthrough",
        ]
        .map(String::from),
    );
    if let Some(filter) = filter {
        args.push("-vf".into());
        args.push(filter);
    }
    args.extend(["-pix_fmt", format.name(), "-f", "rawvideo", "pipe:1"].map(String::from));
    args
}

/// Read the frame at `index` from the decoder's pipe and pair it with its timeline entry; a
/// missing, partial or surplus frame means the decoder and the packet table disagree.
fn read_frame(
    pipe: &mut impl Read,
    bytes: usize,
    timeline: &[(f64, f64)],
    index: usize,
) -> Result<Option<VideoFrame>, MediaError> {
    let mut rgb = vec![0; bytes];
    let filled = fill(pipe, &mut rgb)?;
    match (filled, timeline.get(index)) {
        (0, None) => Ok(None),
        (n, Some(&(time_s, end_s))) if n == bytes && n > 0 => Ok(Some(VideoFrame {
            index: index as u64,
            time_s,
            end_s,
            rgb,
        })),
        _ => Err(disagreement()),
    }
}

/// Read until `buffer` is full or the pipe ends; the number of bytes read.
fn fill(pipe: &mut impl Read, buffer: &mut [u8]) -> Result<usize, MediaError> {
    let mut filled = 0;
    while filled < buffer.len() {
        match pipe.read(&mut buffer[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => return Err(parse_io(error)),
        }
    }
    Ok(filled)
}

/// Whether the pipe still holds output after the last expected frame.
fn has_more(pipe: &mut impl Read) -> Result<bool, MediaError> {
    fill(pipe, &mut [0; 1]).map(|n| n > 0)
}

fn disagreement() -> MediaError {
    MediaError::Parse("decoded frames and presentation timestamps disagree".into())
}

fn parse_io(error: std::io::Error) -> MediaError {
    MediaError::Parse(error.to_string())
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;

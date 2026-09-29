//! Bounded RGB video decoding with source presentation timestamps.
//!
//! **Role:** stream a video's frames from FFmpeg as raw RGB, each paired with its presentation
//! interval from the container's packet table, without retaining the video; `still` decodes a
//! single frame of the same timeline.
//! **Position:** media input for text detection, tracking and validation tools.
//! **Signals and state:** two bounded ffprobe runs before decoding, for the container origin and
//! the video packet table, which the demuxer reads without decoding; then one streaming FFmpeg
//! decoder and its stdout pipe. The stream holds the timeline, one RGB frame at a time and the
//! count of frames handed out.
//! **Invariants:** no frame-rate conversion. The timeline is every decodable packet's
//! presentation time in presentation order (B-frame packets arrive in decode order), and the
//! decoder yields exactly one frame per entry: a missing, partial or surplus frame is an error,
//! never a shifted timestamp. Every timestamp uses the common container origin, preserving the
//! video offset relative to audio. The next presentation timestamp ends a frame; only the final
//! frame uses its reported or fallback duration. Dropping the stream kills the decoder; the
//! source is only read.

pub mod still;

use std::io::{ErrorKind, Read};
use std::path::Path;
use std::process::ChildStdout;
use std::time::Duration;

use crate::{MediaError, Programs};
use child_process::{Run, Running};

/// The most packets a timestamp table may hold: over 92 hours at 24 frames per second.
const MAX_PACKETS: usize = 8_000_000;
/// The longest line a timestamp table may hold.
const MAX_LINE_BYTES: usize = 16 * 1024;
/// ffprobe's key for a packet's presentation time.
const PACKET_TIME: &str = "pts_time";

/// How the decoder is asked to work: exact output, or a fast proxy for screening.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decode {
    /// Every decoding step as encoded: the frames are the source pictures.
    Exact,
    /// The in-loop deblocking filter is skipped where the codec allows it, as H.264 and HEVC do:
    /// faster, with block edges that build up until the next keyframe. For screening only.
    Proxy,
}

/// One decoded frame: its presentation index, origin-relative interval and rgb24 pixels.
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
        if !start_s.is_finite() || !valid_rate(fps) {
            return Err(MediaError::Parse("invalid video timing".into()));
        }
        let origin_s = container_origin(programs, video).unwrap_or(start_s);
        let timeline = packet_timeline(programs, video, origin_s, 1.0 / fps)?;
        let mut decoder = Run::new(&programs.ffmpeg)
            .args(decoder_args(video, size, decode))
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
            "-vf",
        ]
        .map(String::from),
    );
    args.push(format!("scale={}:{}", size.0, size.1));
    args.extend(["-pix_fmt", "rgb24", "-f", "rawvideo", "pipe:1"].map(String::from));
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

/// The format origin is shared with every stream; the caller's offset is only a fallback.
fn container_origin(programs: &Programs, video: &Path) -> Option<f64> {
    let mut running = Run::new(&programs.ffprobe)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=start_time",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
        ])
        .arg(video)
        .timeout(Duration::from_secs(60))
        .spawn()
        .ok()?;
    let mut bytes = Vec::new();
    running
        .take_stdout()?
        .take(1025)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 1024 || running.wait().ok()?.code != 0 {
        return None;
    }
    parse_origin(std::str::from_utf8(&bytes).ok()?)
}

fn parse_origin(text: &str) -> Option<f64> {
    text.trim()
        .parse::<f64>()
        .ok()
        .filter(|time| time.is_finite())
}

/// The presentation timeline from the first video stream's packet table, which ffprobe reads
/// from the container without decoding.
fn packet_timeline(
    programs: &Programs,
    video: &Path,
    origin_s: f64,
    fallback: f64,
) -> Result<Vec<(f64, f64)>, MediaError> {
    let output = Run::new(&programs.ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_packets",
            "-show_entries",
            "packet=pts_time,duration_time,flags",
            "-of",
            "compact=p=0",
        ])
        .arg(video)
        .timeout(Duration::from_secs(600))
        .output()?;
    if output.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffprobe.clone(),
            code: output.code,
            stderr: output.stderr,
        });
    }
    let packets = parse_packets(&output.stdout, fallback, MAX_PACKETS)?;
    presentation_timeline(&packets, origin_s)
}

/// The (time, duration) of every decodable packet, sorted into presentation order. A line
/// without a finite presentation time has no frame, and neither has a packet flagged for the
/// decoder to discard, such as the pre-roll an MP4 edit list skips.
fn parse_packets(table: &str, fallback: f64, limit: usize) -> Result<Vec<(f64, f64)>, MediaError> {
    let mut packets = Vec::new();
    for line in table.lines() {
        if line.len() > MAX_LINE_BYTES {
            return Err(excessive_table());
        }
        if discarded(line) {
            continue;
        }
        if let Some(timing) = parse_timing(line, PACKET_TIME, fallback) {
            if packets.len() == limit {
                return Err(excessive_table());
            }
            packets.push(timing);
        }
    }
    packets.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(packets)
}

fn excessive_table() -> MediaError {
    MediaError::Parse("excessive frame timestamp table".into())
}

/// Whether a packet line's flags carry `D`: the demuxer marks the packet for the decoder to
/// discard, so it produces no frame.
fn discarded(line: &str) -> bool {
    line.trim().split('|').any(|part| {
        part.strip_prefix("flags=")
            .is_some_and(|flags| flags.contains('D'))
    })
}

/// Origin-relative (start, end) of packets in presentation order: each frame ends where the next
/// begins, and the final frame after its own duration.
fn presentation_timeline(
    packets: &[(f64, f64)],
    origin: f64,
) -> Result<Vec<(f64, f64)>, MediaError> {
    packets
        .iter()
        .enumerate()
        .map(|(i, &timing)| frame_interval(timing, packets.get(i + 1).copied(), origin))
        .collect()
}

/// A frame's origin-relative interval; a zero-length or reversed interval is an error.
fn frame_interval(
    timing: (f64, f64),
    next: Option<(f64, f64)>,
    origin: f64,
) -> Result<(f64, f64), MediaError> {
    let (time, duration) = timing;
    let end = next.map_or(time + duration, |(time, _)| time);
    if !end.is_finite() || end <= time {
        return Err(MediaError::Parse(
            "repeated or nonmonotonic frame presentation timestamps".into(),
        ));
    }
    Ok(((time - origin).max(0.0), (end - origin).max(0.0)))
}

/// The time under `time_key` and the duration of one compact ffprobe line, if the time is
/// finite; a missing, non-positive or non-finite duration falls back.
fn parse_timing(line: &str, time_key: &str, fallback: f64) -> Option<(f64, f64)> {
    let mut time = None;
    let mut duration = fallback;
    for part in line.trim().split('|') {
        if let Some((key, value)) = part.split_once('=') {
            if key == time_key {
                time = value.parse::<f64>().ok().filter(|v| v.is_finite());
            } else if key == "duration_time" {
                duration = value
                    .parse::<f64>()
                    .ok()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(fallback);
            }
        }
    }
    time.map(|t| (t, duration))
}

#[cfg(test)]
#[path = "tests/timing.rs"]
mod tests;

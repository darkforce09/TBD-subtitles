//! Full-resolution frames as 8-bit YUV 4:2:0, read into recycled buffers.
//!
//! **Role:** stream a video's frames at their native size, or one even-aligned rectangle of
//! them, as `yuv420p` from the CPU decoder or as nv12 downloaded from NVDEC, each paired with its
//! presentation interval; from the first frame or from any frame index, for all frames or a
//! count.
//! **Position:** beside `FrameStream` in `video_frames`, on the same packet timeline; a
//! `frame_queue::Producer`, so the detection scan, the region source and the localized video
//! run it on a decode thread through `FrameQueue`.
//! **Signals and state:** one FFmpeg decoder whose stdout pipe is enlarged to 1 MiB; a
//! `BufferPool` the frames return to when dropped; the shared timeline and the next index.
//! **Invariants:** frames are always 8-bit (`-pix_fmt yuv420p`, or nv12 from NVDEC) whatever
//! the source's depth; nothing skips a decoding step. A start at frame `first` seeks half a gap
//! before that frame's time, so FFmpeg's accurate seek delivers exactly that frame first. The
//! decoder yields one frame per timeline entry asked for: a missing, partial or surplus frame is
//! an error, never a shifted timestamp, and `finish` fails unless FFmpeg exited cleanly. A
//! reader that stops early ends the decoder; the source is only read.

use std::path::Path;
use std::process::ChildStdout;
use std::sync::Arc;
use std::time::Duration;

use child_process::{Run, Running};
use job_model::outputs::VideoStream;

use super::{PixelFormat, disagreement, fill, has_more, pipe, timeline, valid_rate};
use crate::frame_queue::{BufferPool, FrameQueue, Producer};
use crate::yuv::{ChromaLayout, YuvFrame};
use crate::{MediaError, Programs};

/// How many seconds of frames a decode queue holds ahead of its reader.
pub const QUEUE_SECONDS: f64 = 4.0;
/// The most frame bytes a decode queue holds, whatever the frame rate.
pub const QUEUE_MAX_BYTES: usize = 512 * 1024 * 1024;

/// The deadline of a counted run before its frames: opening the file and seeking.
const BASE_DEADLINE_S: u64 = 120;
/// A counted run's extra deadline per frame; full-resolution decoding runs far faster.
const DEADLINE_PER_FRAME_S: f64 = 0.25;
/// The deadline of a run to the end of the video.
const WHOLE_VIDEO_DEADLINE_S: u64 = 24 * 3600;

/// Which frames a [`YuvStream`] decodes, where, and what part of each.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct YuvOptions {
    /// The presentation index of the first frame.
    pub first: u64,
    /// How many frames from `first`; to the end of the video when `None`.
    pub count: Option<u64>,
    /// Decode on the GPU through NVDEC and download nv12, instead of `yuv420p` from the CPU.
    pub hardware: bool,
    /// The rectangle (x, y, width, height) FFmpeg crops each frame to, every value even.
    pub crop: Option<(u32, u32, u32, u32)>,
    /// How many seconds of frames a queue holds ahead of its reader, defaulting to [`QUEUE_SECONDS`].
    pub queue_seconds: Option<f64>,
    /// The most frame bytes a decode queue holds, defaulting to [`QUEUE_MAX_BYTES`].
    pub queue_max_bytes: Option<usize>,
}

/// Whether NVDEC's nv12 download suits `stream`: an 8-bit 4:2:0 source, whose surfaces download
/// as nv12 without conversion.
pub fn hardware_suits(stream: &VideoStream) -> bool {
    matches!(
        stream.pix_fmt.as_deref(),
        Some("yuv420p" | "yuvj420p" | "nv12")
    )
}

/// Frames of a video's first video stream, in presentation order, as [`YuvFrame`]s.
pub struct YuvStream {
    decoder: Option<Running>,
    pixels: ChildStdout,
    pool: BufferPool,
    timeline: Arc<[(f64, f64)]>,
    size: (u32, u32),
    layout: ChromaLayout,
    depth: usize,
    next: u64,
    end: u64,
    program: String,
}

impl YuvStream {
    /// Read the video's presentation timeline, then start decoding frames of the native
    /// `frame_size` as `options` asks. `start_s` is the origin when the container reports none;
    /// `1 / fps` ends the final frame when its packet reports no duration.
    pub fn open(
        programs: &Programs,
        video: &Path,
        frame_size: (u32, u32),
        start_s: f64,
        fps: f64,
        options: YuvOptions,
    ) -> Result<Self, MediaError> {
        let timeline = timeline(programs, video, start_s, fps)?;
        Self::over(programs, video, frame_size, fps, timeline.into(), options)
    }

    /// Start decoding over a `timeline` read beforehand, as `options` asks; `fps` is the nominal
    /// rate, for a seek to the first frame's neighbour and the queue's depth.
    pub fn over(
        programs: &Programs,
        video: &Path,
        frame_size: (u32, u32),
        fps: f64,
        timeline: Arc<[(f64, f64)]>,
        options: YuvOptions,
    ) -> Result<Self, MediaError> {
        let size = output_size(frame_size, options.crop)?;
        let bytes = PixelFormat::Yuv420p.frame_bytes(size)?;
        let end = span_end(&timeline, options)?;
        if !valid_rate(fps) {
            return Err(MediaError::Parse("invalid frame rate".into()));
        }
        let seek = seek_time(&timeline, options.first, fps);
        let mut decoder = Run::new(&programs.ffmpeg)
            .args(yuv_args(video, seek, options))
            .timeout(deadline(options.count))
            .spawn()?;
        let pixels = decoder
            .take_stdout()
            .ok_or_else(|| MediaError::Parse("no video pipe".into()))?;
        // A pipe that cannot grow keeps its default size: slower, never wrong.
        let _ = pipe::enlarge(&pixels);
        let depth = queue_depth_with(
            fps,
            bytes,
            options.queue_seconds.unwrap_or(QUEUE_SECONDS),
            options.queue_max_bytes.unwrap_or(QUEUE_MAX_BYTES),
        );
        Ok(Self {
            decoder: Some(decoder),
            pixels,
            pool: BufferPool::new(bytes, depth + 2),
            timeline,
            size,
            layout: if options.hardware {
                ChromaLayout::Nv12
            } else {
                ChromaLayout::Planar
            },
            depth,
            next: options.first,
            end,
            program: programs.ffmpeg.clone(),
        })
    }

    /// Origin-relative (time_s, end_s) of every frame of the video, by presentation index.
    pub fn timeline(&self) -> &Arc<[(f64, f64)]> {
        &self.timeline
    }

    /// The width and height of every frame handed out: the crop's, else the video's.
    pub fn frame_size(&self) -> (u32, u32) {
        self.size
    }

    /// How many frames a queue around this stream holds: [`QUEUE_SECONDS`] of frames (or
    /// the custom seconds asked for), within [`QUEUE_MAX_BYTES`] (or the custom byte limit),
    /// and at least two.
    pub fn queue_depth(&self) -> usize {
        self.depth
    }

    /// Run the stream on a decode thread, [`YuvStream::queue_depth`] frames ahead of the reader.
    pub fn spawn(self) -> FrameQueue<YuvFrame, MediaError> {
        let depth = self.queue_depth();
        FrameQueue::spawn(self, depth)
    }

    /// The next frame, or `None` once every frame asked for was handed out.
    pub fn next_frame(&mut self) -> Result<Option<YuvFrame>, MediaError> {
        if self.next == self.end {
            return Ok(None);
        }
        let mut data = self.pool.take();
        let filled = fill(&mut self.pixels, &mut data)?;
        if filled != data.len() {
            return Err(self.ended_early());
        }
        let (time_s, end_s) = self.timeline[self.next as usize];
        let frame = YuvFrame {
            index: self.next,
            time_s,
            end_s,
            width: self.size.0,
            height: self.size.1,
            layout: self.layout,
            data,
        };
        self.next += 1;
        Ok(Some(frame))
    }

    /// Reap the decoder: an error unless it exited cleanly after exactly the frames asked for
    /// were read and nothing more. A caller that stops early drops the stream instead.
    pub fn finish(mut self) -> Result<(), MediaError> {
        let Some(decoder) = self.decoder.take() else {
            return Err(disagreement());
        };
        let complete = self.next == self.end;
        let surplus = if complete {
            has_more(&mut self.pixels)
        } else {
            Ok(false)
        };
        drop(self.pixels);
        let waited = decoder.wait();
        if surplus? {
            return Err(disagreement());
        }
        let finished = waited?;
        if finished.code != 0 {
            return Err(MediaError::Exit {
                program: self.program,
                code: finished.code,
                stderr: finished.stderr,
            });
        }
        if !complete {
            return Err(disagreement());
        }
        Ok(())
    }

    /// The error for a decoder whose output ended inside or before a frame it owed: its own exit
    /// when it failed, else a disagreement with the timeline.
    fn ended_early(&mut self) -> MediaError {
        let Some(decoder) = self.decoder.take() else {
            return disagreement();
        };
        match decoder.wait() {
            Err(error) => error.into(),
            Ok(finished) if finished.code != 0 => MediaError::Exit {
                program: self.program.clone(),
                code: finished.code,
                stderr: finished.stderr,
            },
            Ok(_) => MediaError::Parse(format!(
                "the decoder ended at frame {} of frames up to {}",
                self.next, self.end
            )),
        }
    }
}

impl Producer for YuvStream {
    type Item = YuvFrame;
    type Error = MediaError;

    fn next(&mut self) -> Result<Option<YuvFrame>, MediaError> {
        self.next_frame()
    }

    fn finish(mut self, completed: bool) -> Result<(), MediaError> {
        if completed {
            return YuvStream::finish(self);
        }
        if let Some(decoder) = self.decoder.take() {
            drop(self.pixels);
            let _ = decoder.kill_and_wait();
        }
        Ok(())
    }
}

/// The size of each frame handed out: the crop's when it is even-aligned and inside the frame.
fn output_size(
    frame_size: (u32, u32),
    crop: Option<(u32, u32, u32, u32)>,
) -> Result<(u32, u32), MediaError> {
    let Some((x, y, width, height)) = crop else {
        return Ok(frame_size);
    };
    let even = [x, y, width, height].iter().all(|v| v % 2 == 0);
    let inside = width > 0
        && height > 0
        && x.checked_add(width).is_some_and(|r| r <= frame_size.0)
        && y.checked_add(height).is_some_and(|b| b <= frame_size.1);
    if !even || !inside {
        return Err(MediaError::Parse(format!(
            "crop {width}x{height} at ({x}, {y}) is not an even rectangle inside the {}x{} frame",
            frame_size.0, frame_size.1
        )));
    }
    Ok((width, height))
}

/// One past the last frame index `options` asks for, when its frames are on the timeline.
fn span_end(timeline: &[(f64, f64)], options: YuvOptions) -> Result<u64, MediaError> {
    let frames = timeline.len() as u64;
    let end = match options.count {
        Some(count) if count > 0 => options.first.checked_add(count),
        Some(_) => None,
        None => Some(frames),
    };
    end.filter(|end| options.first < *end && *end <= frames)
        .ok_or_else(|| {
            MediaError::Parse(format!(
                "frames from {} ({:?}) are not on the video's {frames} frames",
                options.first, options.count
            ))
        })
}

/// Where to seek to start at frame `first`: half the gap to the previous frame before its time
/// (`1 / fps` when that gap is unusable), or no seek for the first frame of the video.
fn seek_time(timeline: &[(f64, f64)], first: u64, fps: f64) -> Option<f64> {
    let first = usize::try_from(first).ok().filter(|first| *first > 0)?;
    let time_s = timeline.get(first)?.0;
    let gap = timeline
        .get(first - 1)
        .map(|&(previous_s, _)| time_s - previous_s)
        .filter(|gap| gap.is_finite() && *gap > 0.0)
        .unwrap_or(1.0 / fps);
    Some((time_s - 0.5 * gap).max(0.0))
}

/// Frames a queue holds: [`QUEUE_SECONDS`] at `fps`, within [`QUEUE_MAX_BYTES`] of `bytes`-long
/// frames, at least two.
#[cfg(test)]
fn queue_depth(fps: f64, bytes: usize) -> usize {
    queue_depth_with(fps, bytes, QUEUE_SECONDS, QUEUE_MAX_BYTES)
}

/// Frames a queue holds: `seconds` at `fps`, within `max_bytes` of `bytes`-long frames, at least two.
fn queue_depth_with(fps: f64, bytes: usize, seconds: f64, max_bytes: usize) -> usize {
    let by_time = (seconds * fps).round().max(0.0) as usize;
    let by_bytes = max_bytes / bytes.max(1);
    by_time.min(by_bytes).max(2)
}

/// A counted run's deadline grows with its count; a run to the end has a day.
fn deadline(count: Option<u64>) -> Duration {
    match count {
        Some(count) => {
            let per_frame = (count as f64 * DEADLINE_PER_FRAME_S).min(24.0 * 3600.0);
            Duration::from_secs(BASE_DEADLINE_S) + Duration::from_secs_f64(per_frame)
        }
        None => Duration::from_secs(WHOLE_VIDEO_DEADLINE_S),
    }
}

/// FFmpeg's arguments for the frames `options` asks for, after an accurate seek to `seek` when
/// given, as raw 8-bit 4:2:0 on stdout with no frame-rate conversion.
fn yuv_args(video: &Path, seek: Option<f64>, options: YuvOptions) -> Vec<String> {
    let mut args = Vec::from(["-nostdin", "-hide_banner", "-v", "error"].map(String::from));
    if options.hardware {
        args.extend(["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"].map(String::from));
    }
    if let Some(seek) = seek {
        args.extend(["-seek_timestamp", "0", "-ss"].map(String::from));
        args.push(format!("{seek:.6}"));
    }
    args.extend(["-noautorotate", "-i"].map(String::from));
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
    let crop = options
        .crop
        .map(|(x, y, width, height)| format!("crop={width}:{height}:{x}:{y}"));
    let filter = match (options.hardware, crop) {
        (true, Some(crop)) => Some(format!("hwdownload,format=nv12,{crop}")),
        (true, None) => Some("hwdownload,format=nv12".to_string()),
        (false, crop) => crop,
    };
    if let Some(filter) = filter {
        args.extend(["-vf".to_string(), filter]);
    }
    if let Some(count) = options.count {
        args.extend(["-frames:v".to_string(), count.to_string()]);
    }
    let format = if options.hardware { "nv12" } else { "yuv420p" };
    args.extend(["-pix_fmt", format, "-f", "rawvideo", "pipe:1"].map(String::from));
    args
}

#[cfg(test)]
#[path = "tests/yuv_stream.rs"]
mod tests;

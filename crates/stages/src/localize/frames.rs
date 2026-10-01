//! The localized video's decode thread: the source's frames, in the raw format they are blended
//! and encoded in, decoded ahead of the blend on a thread of their own.
//!
//! **Role:** start FFmpeg on the frames a render asks for and run it on a `FrameQueue` about four
//! seconds of frames ahead of the reader: 8-bit sources through `YuvStream` (from any frame
//! index, into pooled buffers), 10-bit sources through `FrameStream::open_native` filtered to
//! the asked ranges.
//! **Position:** inside `localize`; `render`'s whole-video and segment encodes read it on the
//! step thread through `Decoder::recv`.
//! **Signals and state:** one FFmpeg decoder per `Decoder`, started on the calling thread (which
//! outlives it) and read on the queue's thread; the 10-bit producer holds the ranges still due.
//! **Invariants:** frames arrive in ascending presentation order, exactly the ones asked for; a
//! decoder that ends early or late is an error; dropping a `Decoder` before its end kills
//! FFmpeg; the source is only read.

use std::collections::VecDeque;
use std::path::Path;
use std::sync::Arc;

use media_io::frame_queue::{FrameQueue, PooledBuffer, Producer};
use media_io::video_frames::yuv_stream::{QUEUE_MAX_BYTES, QUEUE_SECONDS};
use media_io::video_frames::{Decode, FrameStream, PixelFormat, YuvOptions, YuvStream};
use media_io::yuv::YuvFrame;
use media_io::{MediaError, Programs};

/// One decoded frame in the render's raw format: its presentation index and its samples.
#[derive(Debug)]
pub struct RawFrame {
    pub index: u64,
    pub data: PooledBuffer,
}

impl From<YuvFrame> for RawFrame {
    fn from(frame: YuvFrame) -> Self {
        RawFrame {
            index: frame.index,
            data: frame.data,
        }
    }
}

/// Frames decoded on a thread of their own, read in order.
pub enum Decoder {
    /// 8-bit 4:2:0 frames from `YuvStream`.
    Yuv(FrameQueue<YuvFrame, MediaError>),
    /// Frames of any raw format from `FrameStream::open_native`, filtered to the asked ranges.
    Native(FrameQueue<RawFrame, MediaError>),
}

impl Decoder {
    /// `count` 8-bit frames (to the end when `None`) from frame `first` of `video`, over its
    /// `timeline` read beforehand.
    pub fn yuv(
        programs: &Programs,
        video: &Path,
        size: (u32, u32),
        fps: f64,
        timeline: Arc<[(f64, f64)]>,
        first: u64,
        count: Option<u64>,
    ) -> Result<Decoder, MediaError> {
        let options = YuvOptions {
            first,
            count,
            ..YuvOptions::default()
        };
        let stream = YuvStream::over(programs, video, size, fps, timeline, options)?;
        Ok(Decoder::Yuv(stream.spawn()))
    }

    /// The frames of `ranges` (ascending, disjoint, both ends included) of `video` in `format`,
    /// decoded from the first frame and handed out only inside the ranges.
    pub fn native(
        programs: &Programs,
        video: &Path,
        size: (u32, u32),
        start_s: f64,
        fps: f64,
        format: PixelFormat,
        ranges: Vec<(u64, u64)>,
    ) -> Result<Decoder, MediaError> {
        let bytes = format.frame_bytes(size)?;
        let stream =
            FrameStream::open_native(programs, video, size, start_s, fps, format, Decode::Exact)?;
        let total = stream.timeline().len() as u64;
        let producer = NativeRanges::new(stream, ranges, total)?;
        Ok(Decoder::Native(FrameQueue::spawn(
            producer,
            queue_depth(fps, bytes),
        )))
    }

    /// The next frame, `None` once every asked frame was handed out, or the decoder's error.
    pub fn recv(&mut self) -> Result<Option<RawFrame>, MediaError> {
        match self {
            Decoder::Yuv(queue) => Ok(queue.recv()?.map(RawFrame::from)),
            Decoder::Native(queue) => queue.recv(),
        }
    }

    /// Stop reading and reap the decoder: its error, if it ended badly.
    pub fn finish(self) -> Result<(), MediaError> {
        let panicked = || MediaError::Parse("the decode thread panicked".into());
        match self {
            Decoder::Yuv(queue) => queue.finish(panicked),
            Decoder::Native(queue) => queue.finish(panicked),
        }
    }
}

/// Frames a native queue holds: [`QUEUE_SECONDS`] at `fps`, within [`QUEUE_MAX_BYTES`] of
/// `bytes`-long frames, at least two.
pub fn queue_depth(fps: f64, bytes: usize) -> usize {
    let by_time = (QUEUE_SECONDS * fps).round().max(0.0) as usize;
    by_time.min(QUEUE_MAX_BYTES / bytes.max(1)).max(2)
}

/// A whole-video native stream handing out only the frames of its ranges.
struct NativeRanges {
    stream: Option<FrameStream>,
    ranges: VecDeque<(u64, u64)>,
    /// Whether the last range ends on the video's last frame, so the stream is read to its end.
    to_the_end: bool,
}

impl NativeRanges {
    fn new(
        stream: FrameStream,
        ranges: Vec<(u64, u64)>,
        total: u64,
    ) -> Result<NativeRanges, MediaError> {
        let ordered = ranges.windows(2).all(|pair| pair[0].1 < pair[1].0);
        let valid = ranges
            .iter()
            .all(|&(first, last)| first <= last && last < total);
        if !ordered || !valid {
            return Err(MediaError::Parse(format!(
                "the frame ranges {ranges:?} are not ascending ranges of the {total} frames"
            )));
        }
        Ok(NativeRanges {
            stream: Some(stream),
            to_the_end: ranges.last().is_some_and(|&(_, last)| last + 1 == total),
            ranges: ranges.into(),
        })
    }
}

impl Producer for NativeRanges {
    type Item = RawFrame;
    type Error = MediaError;

    fn next(&mut self) -> Result<Option<RawFrame>, MediaError> {
        let Some(stream) = self.stream.as_mut() else {
            return Ok(None);
        };
        while let Some(&(first, last)) = self.ranges.front() {
            let Some(frame) = stream.next_frame()? else {
                return Err(MediaError::Parse(format!(
                    "the decoder ended before frame {first}"
                )));
            };
            if frame.index < first {
                continue;
            }
            if frame.index >= last {
                self.ranges.pop_front();
            }
            return Ok(Some(RawFrame {
                index: frame.index,
                data: PooledBuffer::detached(frame.rgb),
            }));
        }
        Ok(None)
    }

    fn finish(mut self, completed: bool) -> Result<(), MediaError> {
        match self.stream.take() {
            // Every frame was read: the decoder must have ended cleanly with nothing left.
            Some(stream) if completed && self.to_the_end => stream.finish(),
            // Stopped before the video's end: dropping the stream kills the decoder.
            _ => Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "tests/frames.rs"]
mod tests;

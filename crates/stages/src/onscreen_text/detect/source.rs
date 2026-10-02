//! Frames for the visual scan: every frame at full resolution in its decoded YUV form, and
//! full-resolution stills for the keyframes the scan did not keep.
//!
//! **Role:** hand the scan every frame of the video with its presentation interval, decoded
//! ahead on a thread of its own, and decode single frames by index when confirmation needs a
//! keyframe the scan released.
//! **Position:** between `media_io::video_frames` and the scan; tests substitute scripted sources.
//! **Signals and state:** one `YuvStream` (yuv420p, or nv12 through NVDEC) running in a
//! `FrameQueue` up to forty-five seconds of frames ahead until `finish`, a copy of the video's
//! timeline, and at most eight concurrent still decoders.
//! **Invariants:** frames arrive in presentation order, one per timeline entry, at the video's
//! own size; a still is the source frame at its timeline time, at source size; an unknown index
//! is an error; the source video is only read.

use std::path::{Path, PathBuf};

use image::RgbImage;
use job_model::outputs::VideoStream;
use media_io::frame_queue::FrameQueue;
use media_io::video_frames::yuv_stream::hardware_suits;
use media_io::video_frames::{YuvOptions, YuvStream, still};
use media_io::yuv::YuvFrame;
use media_io::{MediaError, Programs};

use super::frame_rate;
use crate::onscreen_text::TextResult;

/// Still decoders running at once.
pub(crate) const STILL_DECODERS: usize = 8;
/// How many seconds of frames the scan queue holds ahead of its reader.
const SCAN_QUEUE_SECONDS: f64 = 45.0;
/// The most frame bytes the scan queue holds, keeping within the workstation RAM budget.
const SCAN_QUEUE_MAX_BYTES: usize = 3584 * 1024 * 1024;

/// The frames a scan reads: every frame in order, then full-resolution stills of chosen frames.
pub trait FrameSource: Send {
    /// The decoded frames' width and height: the video's own.
    fn frame_size(&self) -> (u32, u32);
    /// Origin-relative (time_s, end_s) per frame index, known before decoding.
    fn timeline(&self) -> &[(f64, f64)];
    /// The next frame in presentation order, or `None` after the last.
    fn next_frame(&mut self) -> TextResult<Option<YuvFrame>>;
    /// Called once after the last frame; fails when the decoder failed or stopped short.
    fn finish(&mut self) -> TextResult<()>;
    /// Full-resolution rgb24 stills for these frame indices, in the same order.
    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>>;
}

/// A video decoded by FFmpeg: yuv420p frames decoded ahead, and accurately seeked stills.
pub struct FfmpegSource {
    programs: Programs,
    video: PathBuf,
    fps: f64,
    size: (u32, u32),
    timeline: Vec<(f64, f64)>,
    frames: Option<FrameQueue<YuvFrame, MediaError>>,
}

impl FfmpegSource {
    /// Reads the video's timeline and starts decoding every frame at its own size, up to
    /// forty-five seconds ahead of the scan: yuv420p on the CPU, or nv12 through NVDEC when
    /// `hardware` is asked for and the stream's pixel format suits it.
    pub fn open(
        programs: &Programs,
        video: &Path,
        stream: &VideoStream,
        hardware: bool,
    ) -> TextResult<Self> {
        let fps = frame_rate(stream);
        let size = (stream.width, stream.height);
        let options = YuvOptions {
            hardware: hardware && hardware_suits(stream),
            queue_seconds: Some(SCAN_QUEUE_SECONDS),
            queue_max_bytes: Some(SCAN_QUEUE_MAX_BYTES),
            ..YuvOptions::default()
        };
        let frames = YuvStream::open(programs, video, size, stream.start_time_s, fps, options)?;
        let timeline = frames.timeline().to_vec();
        Ok(Self {
            programs: programs.clone(),
            video: video.to_path_buf(),
            fps,
            size,
            timeline,
            frames: Some(frames.spawn()),
        })
    }
}

impl FrameSource for FfmpegSource {
    fn frame_size(&self) -> (u32, u32) {
        self.size
    }

    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn next_frame(&mut self) -> TextResult<Option<YuvFrame>> {
        match self.frames.as_mut() {
            Some(frames) => Ok(frames.recv()?),
            None => Ok(None),
        }
    }

    fn finish(&mut self) -> TextResult<()> {
        match self.frames.take() {
            Some(frames) => Ok(frames
                .finish(|| MediaError::Parse("the frame decoder's thread panicked".into()))?),
            None => Ok(()),
        }
    }

    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>> {
        let times = indices
            .iter()
            .map(|&index| {
                usize::try_from(index)
                    .ok()
                    .and_then(|position| self.timeline.get(position))
                    .map(|&(time_s, _)| time_s)
                    .ok_or_else(|| format!("Frame {index} is not in the video's timeline").into())
            })
            .collect::<TextResult<Vec<f64>>>()?;
        let (programs, video, fps, size) =
            (&self.programs, self.video.as_path(), self.fps, self.size);
        let mut stills = Vec::with_capacity(times.len());
        for chunk in times.chunks(STILL_DECODERS) {
            let decoded: Vec<TextResult<Vec<u8>>> = std::thread::scope(|scope| {
                let workers: Vec<_> = chunk
                    .iter()
                    .map(|&time_s| {
                        scope.spawn(move || still::still(programs, video, time_s, fps, size))
                    })
                    .collect();
                workers
                    .into_iter()
                    .map(|worker| match worker.join() {
                        Ok(result) => result.map_err(Into::into),
                        Err(_) => Err("A still decoder thread panicked".into()),
                    })
                    .collect()
            });
            for rgb in decoded {
                stills.push(
                    RgbImage::from_raw(size.0, size.1, rgb?)
                        .ok_or("A still frame has the wrong size")?,
                );
            }
        }
        Ok(stills)
    }
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;

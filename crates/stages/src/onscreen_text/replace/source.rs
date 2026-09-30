//! Region crops of the source video, decoded at full resolution by FFmpeg.
//!
//! **Role:** hand the replacement stages the pixels of one rectangle over a span of frames, by
//! presentation index, without decoding whole frames into memory.
//! **Position:** the production `RegionSource` over `media_io::video_frames`; tests substitute
//! scripted sources.
//! **Signals and state:** the video's presentation timeline, read once when opened; one
//! `RegionStream` FFmpeg run per `frames` call, holding one crop at a time.
//! **Invariants:** a span is validated against the timeline and a rectangle against the frame
//! before anything runs; crops arrive in index order, one per frame of the span, each the same
//! rectangle of the source frame at its timeline time; the seek margin is half the gap to the
//! previous frame, so a variable frame rate still lands on the first frame of the span; the
//! source video is only read.

use std::path::{Path, PathBuf};

use image::RgbImage;
use job_model::onscreen::PixelRect;
use job_model::outputs::VideoStream;
use media_io::Programs;
use media_io::video_frames::region::RegionStream;
use media_io::video_frames::timeline;

use super::RegionSource;
use crate::onscreen_text::TextResult;

/// The frame rate assumed when the probe reports none.
const FALLBACK_FPS: f64 = 24.0;

/// Regions of a video decoded at full resolution by FFmpeg.
pub struct FfmpegRegions {
    programs: Programs,
    video: PathBuf,
    fps: f64,
    timeline: Vec<(f64, f64)>,
    size: (u32, u32),
}

impl FfmpegRegions {
    /// Read the video's presentation timeline; nothing decodes until `frames`.
    pub fn open(programs: &Programs, video: &Path, stream: &VideoStream) -> TextResult<Self> {
        let fps = frame_rate(stream);
        let timeline = timeline(programs, video, stream.start_time_s, fps)?;
        Ok(Self {
            programs: programs.clone(),
            video: video.to_path_buf(),
            fps,
            timeline,
            size: (stream.width, stream.height),
        })
    }

    /// The seek margin's frame rate for a span starting at `first`: the inverse of the gap to
    /// the previous frame, or the nominal rate for the first frame or a degenerate gap.
    fn seek_rate(&self, first: usize) -> f64 {
        first
            .checked_sub(1)
            .and_then(|previous| self.timeline.get(previous))
            .map(|&(previous_s, _)| self.timeline[first].0 - previous_s)
            .filter(|gap| gap.is_finite() && *gap > 0.0)
            .map_or(self.fps, |gap| 1.0 / gap)
    }
}

impl RegionSource for FfmpegRegions {
    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn frame_size(&self) -> (u32, u32) {
        self.size
    }

    fn frames(
        &mut self,
        rect: PixelRect,
        first: u64,
        last: u64,
        visit: &mut dyn FnMut(u64, RgbImage) -> TextResult<()>,
    ) -> TextResult<()> {
        let start = span_start(first, last, self.timeline.len())?;
        if !inside(rect, self.size) {
            return Err(format!(
                "Region {}x{} at ({}, {}) is not inside the {}x{} frame",
                rect.width, rect.height, rect.x, rect.y, self.size.0, self.size.1
            )
            .into());
        }
        let mut stream = RegionStream::open(
            &self.programs,
            &self.video,
            (rect.x, rect.y, rect.width, rect.height),
            self.timeline[start].0,
            self.seek_rate(start),
            last - first + 1,
        )?;
        for index in first..=last {
            let rgb = stream
                .next_frame()?
                .ok_or_else(|| format!("The region decoder stopped before frame {index}"))?;
            let image = RgbImage::from_raw(rect.width, rect.height, rgb)
                .ok_or("The region decoder returned a crop of the wrong size")?;
            visit(index, image)?;
        }
        Ok(stream.finish()?)
    }
}

/// The timeline position of `first` when `first..=last` is a non-empty span of a
/// `frames`-long timeline.
fn span_start(first: u64, last: u64, frames: usize) -> TextResult<usize> {
    if first > last || last >= frames as u64 {
        return Err(format!(
            "Frames {first} to {last} are not a span of the video's {frames} frames"
        )
        .into());
    }
    Ok(usize::try_from(first)?)
}

/// Whether `rect` is non-empty and wholly inside a frame of `size`, without overflow.
fn inside(rect: PixelRect, size: (u32, u32)) -> bool {
    let right = rect.x.checked_add(rect.width);
    let bottom = rect.y.checked_add(rect.height);
    rect.width > 0
        && rect.height > 0
        && right.is_some_and(|right| right <= size.0)
        && bottom.is_some_and(|bottom| bottom <= size.1)
}

/// The probed frame rate, or 24 when the probe reports none.
fn frame_rate(stream: &VideoStream) -> f64 {
    stream
        .fps()
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .unwrap_or(FALLBACK_FPS)
}

#[cfg(test)]
#[path = "tests/source.rs"]
mod tests;

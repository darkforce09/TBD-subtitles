//! A run of consecutive frames cropped to one region at full resolution.
//!
//! **Role:** decode `count` consecutive source frames from a presentation index, cropped to a
//! rectangle of source pixels, as rgb24 bytes in the stream's own colour, for the stages that
//! measure and replace visible writing in one region of the picture.
//! **Position:** beside `FrameStream` and `still` in `video_frames`, on the same timeline; built
//! on `YuvStream`, and a `frame_queue::Producer`, so the region source runs it on a decode thread.
//! **Signals and state:** one FFmpeg run, through `YuvStream`, whose stdout carries `count`
//! `yuv420p` crops of the even-aligned rectangle around the region, each in a recycled buffer
//! that goes back to the pool once its crop is converted.
//! **Invariants:** FFmpeg crops on even coordinates, so no chroma sample is split; Rust trims the
//! aligned crop to the region and converts it with the stream's matrix and range, so a crop at
//! odd coordinates equals the same rectangle of the whole frame converted alike. The run starts
//! exactly at frame `first`; anything but exactly `count` full crops is an error; a caller that
//! stops early ends FFmpeg; the source is only read.

use std::path::Path;
use std::sync::Arc;

use super::yuv_stream::{YuvOptions, YuvStream};
use crate::frame_queue::{FrameQueue, Producer};
use crate::yuv::{Coefficients, Rect, crop_to_rgb};
use crate::{MediaError, Programs};

/// A region and the even-aligned rectangle FFmpeg crops around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionCrop {
    /// The rectangle (x, y, width, height) FFmpeg crops each frame to: the region with its left
    /// and top edges rounded down to even, its right and bottom up to even, within the frame.
    pub decoded: (u32, u32, u32, u32),
    /// Where the region lies inside the decoded rectangle.
    pub trim: Rect,
}

impl RegionCrop {
    /// The even-aligned rectangle around `region` = (x, y, width, height) in a frame of
    /// `frame_size`, or an error when the region is empty or not inside the frame.
    pub fn around(
        region: (u32, u32, u32, u32),
        frame_size: (u32, u32),
    ) -> Result<RegionCrop, MediaError> {
        let (x, y, width, height) = region;
        let right = x.checked_add(width).filter(|r| *r <= frame_size.0);
        let bottom = y.checked_add(height).filter(|b| *b <= frame_size.1);
        let (Some(right), Some(bottom)) = (right, bottom) else {
            return Err(outside(region, frame_size));
        };
        if width == 0 || height == 0 {
            return Err(outside(region, frame_size));
        }
        let (left, top) = (x & !1, y & !1);
        let right = right.next_multiple_of(2).min(frame_size.0);
        let bottom = bottom.next_multiple_of(2).min(frame_size.1);
        Ok(RegionCrop {
            decoded: (left, top, right - left, bottom - top),
            trim: Rect {
                x: (x - left) as usize,
                y: (y - top) as usize,
                width: width as usize,
                height: height as usize,
            },
        })
    }
}

fn outside(region: (u32, u32, u32, u32), frame_size: (u32, u32)) -> MediaError {
    let (x, y, width, height) = region;
    MediaError::Parse(format!(
        "region {width}x{height} at ({x}, {y}) is not inside the {}x{} frame",
        frame_size.0, frame_size.1
    ))
}

/// Which frames of which region to decode, and in what colour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RegionRequest {
    /// The video's frame size.
    pub frame_size: (u32, u32),
    /// The nominal frame rate, for the seek margin when the gap to the previous frame is unusable.
    pub fps: f64,
    /// The region (x, y, width, height) of source pixels.
    pub region: (u32, u32, u32, u32),
    /// The presentation index of the first frame.
    pub first: u64,
    /// How many consecutive frames.
    pub count: u64,
    /// The stream's matrix and range.
    pub colour: Coefficients,
}

/// One region crop: its frame's presentation index and its rgb24 bytes, width*height*3.
#[derive(Debug)]
pub struct RegionFrame {
    pub index: u64,
    pub rgb: Vec<u8>,
}

/// `count` consecutive frames from `first`, each cropped to one region, as rgb24.
pub struct RegionStream {
    frames: YuvStream,
    trim: Rect,
    colour: Coefficients,
}

impl RegionStream {
    /// Start decoding the frames `request` names over the video's `timeline`. An empty run, a
    /// run past the timeline or a region outside the frame is refused before FFmpeg starts.
    pub fn open(
        programs: &Programs,
        video: &Path,
        timeline: Arc<[(f64, f64)]>,
        request: RegionRequest,
    ) -> Result<Self, MediaError> {
        let crop = RegionCrop::around(request.region, request.frame_size)?;
        let options = YuvOptions {
            first: request.first,
            count: Some(request.count),
            hardware: false,
            crop: Some(crop.decoded),
            ..YuvOptions::default()
        };
        let frames = YuvStream::over(
            programs,
            video,
            request.frame_size,
            request.fps,
            timeline,
            options,
        )?;
        Ok(Self {
            frames,
            trim: crop.trim,
            colour: request.colour,
        })
    }

    /// Run the stream on a decode thread, converting each crop there, at most
    /// [`YuvStream::queue_depth`] crops ahead of the reader.
    pub fn spawn(self) -> FrameQueue<RegionFrame, MediaError> {
        let depth = self.frames.queue_depth();
        FrameQueue::spawn(self, depth)
    }

    /// The next crop, or `None` once `count` crops were read; a partial or missing crop is an
    /// error.
    pub fn next_frame(&mut self) -> Result<Option<RegionFrame>, MediaError> {
        let Some(frame) = self.frames.next_frame()? else {
            return Ok(None);
        };
        let picture = frame.picture().ok_or_else(|| {
            MediaError::Parse("the region decoder returned a crop of the wrong size".into())
        })?;
        let mut rgb = Vec::with_capacity(self.trim.width * self.trim.height * 3);
        if !crop_to_rgb(&picture, &self.colour, self.trim, &mut rgb) {
            return Err(MediaError::Parse(
                "the region lies outside its decoded crop".into(),
            ));
        }
        Ok(Some(RegionFrame {
            index: frame.index,
            rgb,
        }))
    }

    /// Reap FFmpeg: an error unless it exited cleanly after exactly `count` crops were read.
    pub fn finish(self) -> Result<(), MediaError> {
        self.frames.finish()
    }
}

impl Producer for RegionStream {
    type Item = RegionFrame;
    type Error = MediaError;

    fn next(&mut self) -> Result<Option<RegionFrame>, MediaError> {
        self.next_frame()
    }

    fn finish(self, completed: bool) -> Result<(), MediaError> {
        Producer::finish(self.frames, completed)
    }
}

#[cfg(test)]
#[path = "tests/region.rs"]
mod tests;

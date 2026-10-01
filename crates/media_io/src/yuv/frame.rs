//! One decoded YUV 4:2:0 frame with its place on the video's timeline, in a recycled buffer.
//!
//! **Role:** carry a frame from the decoder thread to whoever reads it: its presentation index
//! and times, its size and chroma layout, and its samples.
//!
//! **Position:** produced by `video_frames::yuv_stream` and consumed by the detection scan, the
//! region source and the localized video.
//!
//! **Signals and state:** the samples live in a `PooledBuffer` that goes back to its pool when
//! the frame is dropped.
//!
//! **Invariants:** `data` holds exactly one picture of `width` × `height` in `layout`.

use super::convert::Yuv420;
use crate::frame_queue::PooledBuffer;

/// How a frame's chroma samples are laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaLayout {
    /// yuv420p: U then V planes, from the CPU decoder.
    Planar,
    /// nv12: interleaved U and V, from NVDEC.
    Nv12,
}

/// A decoded frame at its presentation index.
#[derive(Debug)]
pub struct YuvFrame {
    /// The frame's position in presentation order, from zero.
    pub index: u64,
    pub time_s: f64,
    pub end_s: f64,
    pub width: u32,
    pub height: u32,
    pub layout: ChromaLayout,
    pub data: PooledBuffer,
}

impl YuvFrame {
    /// The frame's samples as a picture, or `None` when the buffer does not hold one.
    pub fn picture(&self) -> Option<Yuv420<'_>> {
        let (width, height) = (self.width as usize, self.height as usize);
        match self.layout {
            ChromaLayout::Planar => Yuv420::planar(&self.data, width, height),
            ChromaLayout::Nv12 => Yuv420::nv12(&self.data, width, height),
        }
    }
}

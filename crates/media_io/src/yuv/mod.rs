//! YUV 4:2:0 frames in Rust: the source's matrix and range, conversion to R′G′B′ whole, padded
//! or cropped, and what is read from the brightness plane alone.
//!
//! **Role:** let FFmpeg hand over frames in their decoded YUV form, half the bytes of rgb24, and
//! convert only the frames and rectangles that need colour, in the stream's own colour.
//!
//! **Position:** used by `video_frames::yuv_stream` for its frames, and by the detection scan,
//! the region source, the localized video and the detection benchmark.
//!
//! **Signals and state:** none; plain values and caller-owned buffers.
//!
//! **Invariants:** 8-bit samples only, a 10-bit source being decoded to 8 bits by FFmpeg first;
//! integer conversion in 13-bit fixed point, identical on one thread or many.

mod colour;
mod convert;
mod frame;
mod luma;

pub use colour::{Coefficients, Matrix, Range};
pub use convert::{
    Chroma, Rect, Yuv420, crop_to_rgb, picture_bytes, to_rgb, to_rgb_padded_into, to_rgb_parallel,
};
pub use frame::{ChromaLayout, YuvFrame};
pub use luma::{LumaThumbnail, crop_grey, luma_thumbnail};

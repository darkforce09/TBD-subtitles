//! Every frame at its native size in a chosen raw pixel format.
//!
//! **Role:** stream a video's frames unscaled, in the pixel format an encoder reads, with the same
//! one-frame-per-timeline-entry guarantee as the rgb24 stream.
//! **Position:** a second constructor of `FrameStream`, for the localized-video encoder and the
//! frame compositing before it.
//! **Signals and state:** as `FrameStream`: the packet timeline, then one FFmpeg decoder whose
//! stdout carries `PixelFormat::frame_bytes` bytes per frame.
//! **Invariants:** no scale filter and no frame-rate conversion; FFmpeg converts only the pixel
//! format. A 4:2:0 format needs an even width and height, else it is refused before anything runs.

use std::path::Path;

use super::{Decode, FrameStream, stream_args, timeline};
use crate::{MediaError, Programs};

/// The largest frame a stream hands out, in bytes.
const MAX_FRAME_BYTES: usize = 256 * 1024 * 1024;

/// A raw pixel layout FFmpeg reads and writes on a pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// Packed 8-bit red, green, blue: three bytes per pixel.
    Rgb24,
    /// Planar 8-bit luma and quarter-size chroma planes: a byte and a half per pixel.
    Yuv420p,
    /// As `Yuv420p` with 10-bit samples in little-endian 16-bit words: three bytes per pixel.
    Yuv420p10le,
}

impl PixelFormat {
    /// FFmpeg's name for the format, as `-pix_fmt` takes it.
    pub fn name(self) -> &'static str {
        match self {
            PixelFormat::Rgb24 => "rgb24",
            PixelFormat::Yuv420p => "yuv420p",
            PixelFormat::Yuv420p10le => "yuv420p10le",
        }
    }

    /// Whether the samples are wider than 8 bits.
    pub fn is_high_bit_depth(self) -> bool {
        self == PixelFormat::Yuv420p10le
    }

    /// The byte size of one frame at `size`: non-zero, at most 256 MiB, and for a 4:2:0 format an
    /// even width and height.
    pub fn frame_bytes(self, size: (u32, u32)) -> Result<usize, MediaError> {
        let (width, height) = (size.0 as usize, size.1 as usize);
        let subsampled = matches!(self, PixelFormat::Yuv420p | PixelFormat::Yuv420p10le);
        if subsampled && (width % 2 != 0 || height % 2 != 0) {
            return Err(MediaError::Parse(format!(
                "{} needs an even frame size, not {width}x{height}",
                self.name()
            )));
        }
        let pixels = width.checked_mul(height);
        let bytes = match self {
            PixelFormat::Rgb24 | PixelFormat::Yuv420p10le => pixels.and_then(|p| p.checked_mul(3)),
            PixelFormat::Yuv420p => pixels.and_then(|p| p.checked_mul(3)).map(|b| b / 2),
        };
        bytes
            .filter(|b| *b > 0 && *b <= MAX_FRAME_BYTES)
            .ok_or_else(|| MediaError::Parse("invalid or excessive video dimensions".into()))
    }
}

impl FrameStream {
    /// Read the video's presentation timeline, then start decoding every frame at its native
    /// `size` (the probed width and height) in `format`.
    ///
    /// `start_s` and `fps` are as for [`FrameStream::open`].
    pub fn open_native(
        programs: &Programs,
        video: &Path,
        size: (u32, u32),
        start_s: f64,
        fps: f64,
        format: PixelFormat,
        decode: Decode,
    ) -> Result<Self, MediaError> {
        let bytes = format.frame_bytes(size)?;
        let timeline = timeline(programs, video, start_s, fps)?;
        Self::start(
            programs,
            native_args(video, format, decode),
            bytes,
            timeline,
        )
    }
}

/// FFmpeg's arguments for every frame of the first video stream at its native size, as raw
/// `format` pixels on stdout with no frame-rate conversion.
pub(super) fn native_args(video: &Path, format: PixelFormat, decode: Decode) -> Vec<String> {
    stream_args(video, None, format, decode)
}

#[cfg(test)]
#[path = "tests/native.rs"]
mod tests;

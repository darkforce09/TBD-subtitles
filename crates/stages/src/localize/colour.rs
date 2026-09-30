//! RGB to Y′CbCr conversion in the source video's own matrix, range and bit depth.
//!
//! **Role:** turn a patch's 8-bit R′G′B′ pixels into the sample values of the decoded frames, so
//! a blended patch shows the colour it was composed in.
//! **Position:** used by `patches` when a patch is loaded; chosen once per render from the probe.
//! **Signals and state:** plain values; no I/O.
//! **Invariants:** the matrix follows the stream's `color_space` tag, else its height (BT.709 from
//! 720 lines, BT.601 below); the range follows `color_range` (`pc` is full, anything else
//! limited); every value is clamped to the sample range of its bit depth.

use job_model::outputs::VideoStream;
use media_io::video_frames::PixelFormat;

/// The lowest frame height taken as high definition when the matrix is not tagged.
const HD_LINES: u32 = 720;

/// The luma coefficients of a Y′CbCr matrix.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Matrix {
    /// ITU-R BT.709: high definition.
    Bt709,
    /// ITU-R BT.601 (`bt470bg`, `smpte170m`): standard definition.
    Bt601,
    /// ITU-R BT.2020 non-constant luminance.
    Bt2020,
}

impl Matrix {
    /// The red and blue luma weights (Kr, Kb).
    fn weights(self) -> (f64, f64) {
        match self {
            Matrix::Bt709 => (0.2126, 0.0722),
            Matrix::Bt601 => (0.299, 0.114),
            Matrix::Bt2020 => (0.2627, 0.0593),
        }
    }

    /// The matrix a stream's frames use: its tag when known, else by frame height.
    pub fn of(stream: &VideoStream) -> Matrix {
        match stream.color_space.as_deref() {
            Some("bt709") => Matrix::Bt709,
            Some("bt470bg" | "smpte170m") => Matrix::Bt601,
            Some("bt2020nc" | "bt2020c") => Matrix::Bt2020,
            _ if stream.height >= HD_LINES => Matrix::Bt709,
            _ => Matrix::Bt601,
        }
    }
}

/// How sample values map onto the signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Range {
    /// Studio swing: 16–235 luma and 16–240 chroma at 8 bits.
    Limited,
    /// Full swing: 0–255 at 8 bits.
    Full,
}

impl Range {
    /// The range a stream's frames use: full for `pc`, else limited.
    pub fn of(stream: &VideoStream) -> Range {
        match stream.color_range.as_deref() {
            Some("pc") => Range::Full,
            _ => Range::Limited,
        }
    }
}

/// The conversion from 8-bit R′G′B′ to one frame format's Y′CbCr samples.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conversion {
    pub matrix: Matrix,
    pub range: Range,
    /// Bits per sample: 8 or 10.
    pub bits: u32,
}

impl Conversion {
    /// The conversion for frames of `stream` decoded as `format`.
    pub fn of(stream: &VideoStream, format: PixelFormat) -> Conversion {
        Conversion {
            matrix: Matrix::of(stream),
            range: Range::of(stream),
            bits: if format.is_high_bit_depth() { 10 } else { 8 },
        }
    }

    /// The largest sample value.
    pub fn max_sample(self) -> u16 {
        ((1u32 << self.bits) - 1) as u16
    }

    /// The unrounded (Y, Cb, Cr) sample values of an 8-bit R′G′B′ colour.
    pub fn yuv(self, rgb: [u8; 3]) -> [f64; 3] {
        let (kr, kb) = self.matrix.weights();
        let [r, g, b] = rgb.map(|v| f64::from(v) / 255.0);
        let y = kr * r + (1.0 - kr - kb) * g + kb * b;
        let pb = (b - y) / (2.0 * (1.0 - kb));
        let pr = (r - y) / (2.0 * (1.0 - kr));
        let max = f64::from(self.max_sample());
        let scale = f64::from(1u32 << (self.bits - 8));
        let middle = f64::from(1u32 << (self.bits - 1));
        let (luma, chroma) = match self.range {
            Range::Limited => (
                (16.0 + 219.0 * y) * scale,
                [224.0 * pb, 224.0 * pr].map(|c| c * scale + middle),
            ),
            Range::Full => (y * max, [pb * max + middle, pr * max + middle]),
        };
        [luma, chroma[0], chroma[1]].map(|value| value.clamp(0.0, max))
    }

    /// The (Y, Cb, Cr) sample values of an 8-bit R′G′B′ colour, rounded to the nearest sample.
    pub fn samples(self, rgb: [u8; 3]) -> [u16; 3] {
        self.yuv(rgb).map(|value| value.round() as u16)
    }

    /// The 8-bit R′G′B′ colour of (Y, Cb, Cr) sample values: the inverse of [`Conversion::yuv`],
    /// rounded and clamped to 0–255.
    pub fn rgb(self, samples: [u16; 3]) -> [u8; 3] {
        let (kr, kb) = self.matrix.weights();
        let [luma, cb, cr] = samples.map(f64::from);
        let max = f64::from(self.max_sample());
        let scale = f64::from(1u32 << (self.bits - 8));
        let middle = f64::from(1u32 << (self.bits - 1));
        let (y, pb, pr) = match self.range {
            Range::Limited => (
                (luma / scale - 16.0) / 219.0,
                (cb - middle) / scale / 224.0,
                (cr - middle) / scale / 224.0,
            ),
            Range::Full => (luma / max, (cb - middle) / max, (cr - middle) / max),
        };
        let r = y + 2.0 * (1.0 - kr) * pr;
        let b = y + 2.0 * (1.0 - kb) * pb;
        let g = (y - kr * r - kb * b) / (1.0 - kr - kb);
        [r, g, b].map(|value| (value * 255.0).round().clamp(0.0, 255.0) as u8)
    }
}

#[cfg(test)]
#[path = "tests/colour.rs"]
mod tests;

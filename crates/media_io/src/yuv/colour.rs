//! The Y′CbCr matrix and range a stream's frames use, and the fixed-point coefficients that turn
//! its 8-bit samples into R′G′B′.
//!
//! **Role:** name the matrix and range from the stream's tags, and derive integer coefficients
//! for each pair, so every conversion in Rust follows the source's own colour.
//!
//! **Position:** used by `yuv::convert`; `stages::localize::colour` builds its R′G′B′-to-Y′CbCr
//! conversion on the same matrix and range.
//!
//! **Signals and state:** plain values.
//!
//! **Invariants:** the matrix follows the stream's `color_space` tag, else its height (BT.709
//! from 720 lines, BT.601 below); the range follows `color_range` (`pc` is full, anything else
//! limited); coefficients are in 13-bit fixed point and every channel is rounded and clamped to
//! 0–255.

use job_model::outputs::VideoStream;

/// The lowest frame height taken as high definition when the matrix is not tagged.
const HD_LINES: u32 = 720;

/// The fixed-point scale of the coefficients: 2^13.
const SCALE: f64 = 8192.0;

/// The luma coefficients of a Y′CbCr matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn weights(self) -> (f64, f64) {
        match self {
            Matrix::Bt709 => (0.2126, 0.0722),
            Matrix::Bt601 => (0.299, 0.114),
            Matrix::Bt2020 => (0.2627, 0.0593),
        }
    }

    /// The matrix a stream's frames use: its tag when known, else by frame height.
    pub fn of(stream: &VideoStream) -> Matrix {
        Matrix::from_tag(stream.color_space.as_deref(), stream.height)
    }

    /// The matrix of an ffprobe `color_space` tag, else by frame height.
    pub fn from_tag(color_space: Option<&str>, height: u32) -> Matrix {
        match color_space {
            Some("bt709") => Matrix::Bt709,
            Some("bt470bg" | "smpte170m") => Matrix::Bt601,
            Some("bt2020nc" | "bt2020c") => Matrix::Bt2020,
            _ if height >= HD_LINES => Matrix::Bt709,
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
        Range::from_tag(stream.color_range.as_deref())
    }

    /// The range of an ffprobe `color_range` tag: full for `pc`, else limited.
    pub fn from_tag(color_range: Option<&str>) -> Range {
        match color_range {
            Some("pc") => Range::Full,
            _ => Range::Limited,
        }
    }

    /// The 8-bit luma sample shown as black.
    fn luma_black(self) -> i32 {
        match self {
            Range::Limited => 16,
            Range::Full => 0,
        }
    }
}

/// The integer conversion from one matrix and range's 8-bit Y′CbCr to 8-bit R′G′B′.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coefficients {
    luma_black: i32,
    luma: i32,
    red_v: i32,
    green_u: i32,
    green_v: i32,
    blue_u: i32,
}

impl Coefficients {
    /// The coefficients of `matrix` in `range`.
    pub fn new(matrix: Matrix, range: Range) -> Coefficients {
        let (kr, kb) = matrix.weights();
        let kg = 1.0 - kr - kb;
        let (luma, chroma) = match range {
            Range::Limited => (255.0 / 219.0, 255.0 / 224.0),
            Range::Full => (1.0, 1.0),
        };
        let fixed = |value: f64| (value * SCALE).round() as i32;
        Coefficients {
            luma_black: range.luma_black(),
            luma: fixed(luma),
            red_v: fixed(2.0 * (1.0 - kr) * chroma),
            green_u: fixed(2.0 * (1.0 - kb) * kb / kg * chroma),
            green_v: fixed(2.0 * (1.0 - kr) * kr / kg * chroma),
            blue_u: fixed(2.0 * (1.0 - kb) * chroma),
        }
    }

    /// The coefficients of a stream's own matrix and range.
    pub fn of(stream: &VideoStream) -> Coefficients {
        Coefficients::new(Matrix::of(stream), Range::of(stream))
    }

    /// One pixel's R′G′B′, rounded and clamped to 0–255.
    #[inline]
    pub fn rgb(&self, y: u8, u: u8, v: u8) -> [u8; 3] {
        const ROUND: i32 = 1 << 12;
        let y = (i32::from(y) - self.luma_black) * self.luma;
        let u = i32::from(u) - 128;
        let v = i32::from(v) - 128;
        let channel = |value: i32| ((value + ROUND) >> 13).clamp(0, 255) as u8;
        [
            channel(y + self.red_v * v),
            channel(y - self.green_u * u - self.green_v * v),
            channel(y + self.blue_u * u),
        ]
    }

    /// A luma sample as a full-range grey level: what a grey conversion of the pixel's R′G′B′
    /// would give, within rounding.
    #[inline]
    pub fn grey(&self, y: u8) -> u8 {
        const ROUND: i32 = 1 << 12;
        (((i32::from(y) - self.luma_black) * self.luma + ROUND) >> 13).clamp(0, 255) as u8
    }
}

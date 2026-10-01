//! The H.264 profiles and levels a re-encoded segment can match, and each level's rate limits.
//!
//! **Role:** name a source's profile and level as ffprobe reports them and as the encoders take
//! them, and give the peak rate and buffer a segment may use: a share of the source's rate, never
//! above the level's maximum.
//! **Position:** inside `encode::segments`; the source probe reads profiles and levels through it
//! and the segment arguments write them.
//! **Signals and state:** constant tables; holds nothing.
//! **Invariants:** only progressive 4:2:0 profiles are matched; a level outside Table A-1 of the
//! H.264 specification is unknown, never guessed; the peak rate and buffer never exceed the
//! level's limits scaled by the profile's factor.

use super::super::H264_PEAK_SHARE;

/// An H.264 profile a segment encoder can produce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum H264Profile {
    /// Baseline or Constrained Baseline: no B-frames, no CABAC.
    Baseline,
    Main,
    High,
    /// High 10: the 10-bit 4:2:0 profile, which only x264 encodes here.
    High10,
}

impl H264Profile {
    /// The profile ffprobe names; `None` for one a segment cannot match, such as High 4:2:2.
    pub fn from_ffprobe(name: &str) -> Option<H264Profile> {
        match name.trim() {
            "Baseline" | "Constrained Baseline" => Some(H264Profile::Baseline),
            "Main" => Some(H264Profile::Main),
            "High" | "Constrained High" => Some(H264Profile::High),
            "High 10" => Some(H264Profile::High10),
            _ => None,
        }
    }

    /// The name libx264's `-profile:v` takes.
    pub fn x264_name(self) -> &'static str {
        match self {
            H264Profile::Baseline => "baseline",
            H264Profile::Main => "main",
            H264Profile::High => "high",
            H264Profile::High10 => "high10",
        }
    }

    /// The name h264_nvenc's `-profile:v` takes; NVENC has no 10-bit H.264 profile.
    pub fn nvenc_name(self) -> Option<&'static str> {
        match self {
            H264Profile::Baseline => Some("baseline"),
            H264Profile::Main => Some("main"),
            H264Profile::High => Some("high"),
            H264Profile::High10 => None,
        }
    }

    /// Whether the profile carries 10-bit samples.
    pub fn is_ten_bit(self) -> bool {
        self == H264Profile::High10
    }

    /// The factor Table A-1's rate and buffer limits are multiplied by (`cpbBrVclFactor`).
    fn rate_factor(self) -> u64 {
        match self {
            H264Profile::Baseline | H264Profile::Main => 1000,
            H264Profile::High => 1250,
            H264Profile::High10 => 3000,
        }
    }
}

/// An H.264 level, as the `level_idc` ffprobe reports (40 is level 4.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct H264Level(pub u32);

/// Table A-1: each `level_idc` with its maximum video bit rate (`MaxBR`) and coded picture
/// buffer (`MaxCPB`), both in units of the profile's rate factor. `level_idc` 9 is level 1b.
const LEVEL_LIMITS: [(u32, u64, u64); 20] = [
    (9, 128, 350),
    (10, 64, 175),
    (11, 192, 500),
    (12, 384, 1_000),
    (13, 768, 2_000),
    (20, 2_000, 2_000),
    (21, 4_000, 4_000),
    (22, 4_000, 4_000),
    (30, 10_000, 10_000),
    (31, 14_000, 14_000),
    (32, 20_000, 20_000),
    (40, 20_000, 25_000),
    (41, 50_000, 62_500),
    (42, 50_000, 62_500),
    (50, 135_000, 135_000),
    (51, 240_000, 240_000),
    (52, 240_000, 240_000),
    (60, 240_000, 240_000),
    (61, 480_000, 480_000),
    (62, 800_000, 800_000),
];

impl H264Level {
    /// Whether Table A-1 lists the level.
    pub fn is_known(self) -> bool {
        self.limits().is_some()
    }

    /// The name `-level` takes for both encoders: `4.0`, `3.1`, or `1b`.
    pub fn name(self) -> String {
        match self.0 {
            9 => "1b".into(),
            idc => format!("{}.{}", idc / 10, idc % 10),
        }
    }

    /// The level's (`MaxBR`, `MaxCPB`) in Table A-1's units.
    fn limits(self) -> Option<(u64, u64)> {
        LEVEL_LIMITS
            .iter()
            .find(|&&(idc, _, _)| idc == self.0)
            .map(|&(_, rate, buffer)| (rate, buffer))
    }
}

/// The rate limits a segment encodes under, in bits per second and bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PeakRate {
    /// The peak rate, `-maxrate`.
    pub max_bits_per_s: u64,
    /// The rate control buffer, `-bufsize`.
    pub buffer_bits: u64,
}

/// The peak rate a segment may use: `H264_PEAK_SHARE` times the source's rate when it is known,
/// capped at the level's maximum, with a buffer of twice the peak capped at the level's buffer;
/// `None` for a level Table A-1 does not list.
pub fn peak_rate(
    source_bit_rate: Option<u64>,
    profile: H264Profile,
    level: H264Level,
) -> Option<PeakRate> {
    let (rate, buffer) = level.limits()?;
    let level_rate = rate * profile.rate_factor();
    let level_buffer = buffer * profile.rate_factor();
    let peak = source_bit_rate
        .filter(|&source| source > 0)
        .map(|source| (source as f64 * H264_PEAK_SHARE).round() as u64)
        .map_or(level_rate, |share| share.min(level_rate));
    Some(PeakRate {
        max_bits_per_s: peak,
        buffer_bits: (2 * peak).min(level_buffer),
    })
}

#[cfg(test)]
#[path = "tests/profile.rs"]
mod tests;

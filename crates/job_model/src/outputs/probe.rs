//! What ffprobe found in a video: its duration, its video stream and its audio tracks.
//!
//! **Role:** describe the video's streams for every step that decodes it.
//!
//! **Position:** returned by `media_io`; kept in `probe.json` by the probe-and-decode step.
//!
//! **Signals and state:** none; plain data, written as JSON and archived with rkyv.
//!
//! **Invariants:** a stream field ffprobe does not report is `None`, never a guessed value.

use serde::{Deserialize, Serialize};

/// The probe result kept in `job.json`.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ProbeResult {
    /// The container's duration in seconds.
    pub duration_s: f64,
    /// The first video stream, when there is one.
    pub video: Option<VideoStream>,
    /// Every audio stream, in file order.
    pub audio: Vec<AudioStream>,
}

/// One video stream.
#[derive(
    Debug,
    Clone,
    Default,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct VideoStream {
    pub index: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    /// The frame rate as a fraction, such as 24/1 or 24000/1001.
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub start_time_s: f64,
    /// FFmpeg's name for the decoded pixel format, such as `yuv420p10le`; `None` when ffprobe
    /// reports none or the probe predates the field.
    #[serde(default)]
    pub pix_fmt: Option<String>,
    /// The colour primaries tag, such as `bt709`; `None` when unknown.
    #[serde(default)]
    pub color_primaries: Option<String>,
    /// The transfer characteristics tag, such as `bt709`; `None` when unknown.
    #[serde(default)]
    pub color_transfer: Option<String>,
    /// The matrix coefficients tag, such as `bt709`; `None` when unknown.
    #[serde(default)]
    pub color_space: Option<String>,
    /// The sample range, `tv` (limited) or `pc` (full); `None` when unknown.
    #[serde(default)]
    pub color_range: Option<String>,
    /// The stream's average bit rate in bits per second, else the whole file's; `None` when
    /// ffprobe reports neither.
    #[serde(default)]
    pub bit_rate: Option<u64>,
}

/// One audio stream.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct AudioStream {
    /// The stream index in the file, as ffprobe numbers it.
    pub index: u32,
    /// The position among the audio streams only, as `-map 0:a:<n>` counts.
    pub audio_position: u32,
    pub codec: String,
    /// The language tag, such as `eng`; `None` when absent or `und`.
    pub language: Option<String>,
    pub channels: u32,
    pub sample_rate: u32,
    pub start_time_s: f64,
}

impl VideoStream {
    /// Frames per second, or `None` for a zero denominator.
    pub fn fps(&self) -> Option<f64> {
        (self.frame_rate_den != 0).then(|| self.frame_rate_num as f64 / self.frame_rate_den as f64)
    }
}

/// What the probe-and-decode step keeps in `probe.json`: the probe, the audio track it decoded,
/// and how many 16 kHz samples the mix file holds.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Serialize,
    Deserialize,
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
)]
pub struct ProbeDecoded {
    pub probe: ProbeResult,
    pub track: AudioStream,
    pub samples: u64,
}

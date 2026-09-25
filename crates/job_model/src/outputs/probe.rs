//! What ffprobe found in a video: its duration, its video stream and its audio tracks.

use serde::{Deserialize, Serialize};

/// The probe result kept in `job.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeResult {
    /// The container's duration in seconds.
    pub duration_s: f64,
    /// The first video stream, when there is one.
    pub video: Option<VideoStream>,
    /// Every audio stream, in file order.
    pub audio: Vec<AudioStream>,
}

/// One video stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VideoStream {
    pub index: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    /// The frame rate as a fraction, such as 24/1 or 24000/1001.
    pub frame_rate_num: u32,
    pub frame_rate_den: u32,
    pub start_time_s: f64,
}

/// One audio stream.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

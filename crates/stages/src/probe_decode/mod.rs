//! Probe and decode: ffprobe the video, choose the audio track, and stream the mix to a 16 kHz
//! mono `f32` file the engines and the detector read.
//!
//! **Role:** the first step of every job; the probe result gives the duration and the frame rate
//! every later step uses.
//!
//! **Position:** called by the probe-and-decode step inside a worker process of the app binary,
//! which measures FFmpeg's memory as its child's; uses `media_io`.
//!
//! **Signals and state:** runs ffprobe and FFmpeg; writes the mix file (part file, then rename).
//!
//! **Invariants:** the video is only read; the mix is never held in memory whole.

use std::path::Path;
use std::time::Duration;

use job_model::outputs::{AudioStream, ProbeResult};
use media_io::pcm_stream::{PcmFormat, PcmRequest, PcmStream, write_f32_file};
use media_io::{MediaError, Programs, probe};

/// Frames per chunk read from FFmpeg: one second at 16 kHz.
pub const CHUNK_FRAMES: usize = 16_000;
/// The longest a probe and decode may take.
pub const DEADLINE: Duration = Duration::from_secs(3600);

/// What the step found and wrote.
#[derive(Debug, Clone, PartialEq)]
pub struct Decoded {
    pub probe: ProbeResult,
    pub track: AudioStream,
    /// Samples written to the mix file.
    pub samples: u64,
}

/// The audio track to use: the one at `position` among the audio streams when given, else the
/// English one.
pub fn pick_track(probe: &ProbeResult, position: Option<u32>) -> Result<&AudioStream, MediaError> {
    match position {
        Some(p) => probe
            .audio
            .iter()
            .find(|a| a.audio_position == p)
            .ok_or_else(|| MediaError::Parse(format!("the video has no audio track {p}"))),
        None => probe::english_track(probe),
    }
}

/// Probe `video` and write its mix, as 16 kHz mono `f32`, to `mix`.
pub fn probe_and_decode(
    programs: &Programs,
    video: &Path,
    position: Option<u32>,
    mix: &Path,
) -> Result<Decoded, MediaError> {
    let probe = probe::probe(programs, video)?;
    let track = pick_track(&probe, position)?.clone();
    let request = PcmRequest {
        video,
        audio_position: track.audio_position,
        format: PcmFormat::MONO_16K,
        window: None,
        chunk_frames: CHUNK_FRAMES,
        deadline: DEADLINE,
    };
    let stream = PcmStream::open(programs, &request)?;
    let samples = write_f32_file(stream, mix)?;
    Ok(Decoded {
        probe,
        track,
        samples,
    })
}

#[cfg(test)]
#[path = "tests/probe_decode.rs"]
mod tests;

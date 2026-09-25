//! ffprobe's JSON for a video, read into a [`ProbeResult`], and the choice of the English track.
//!
//! **Role:** run ffprobe on a video, read its streams into the probe result, and pick the audio
//! track to transcribe.
//!
//! **Position:** called by the probe-and-decode stage and the stack spike tool; runs `ffprobe`
//! through `child_process`.
//!
//! **Signals and state:** reads the video through ffprobe; holds nothing.
//!
//! **Invariants:** `und` is no language; several untagged tracks are refused, never guessed.

use std::path::Path;
use std::time::Duration;

use child_process::Run;
use job_model::outputs::{AudioStream, ProbeResult, VideoStream};
use serde::Deserialize;

use crate::{MediaError, Programs};

/// ffprobe reads only headers; a minute means something is wrong.
const PROBE_DEADLINE: Duration = Duration::from_secs(60);

/// Probe `video` with ffprobe.
pub fn probe(programs: &Programs, video: &Path) -> Result<ProbeResult, MediaError> {
    let out = Run::new(&programs.ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(video)
        .timeout(PROBE_DEADLINE)
        .output()?;
    if out.code != 0 {
        return Err(MediaError::Exit {
            program: programs.ffprobe.clone(),
            code: out.code,
            stderr: out.stderr,
        });
    }
    parse(&out.stdout)
}

/// Read ffprobe's `-print_format json -show_format -show_streams` output.
pub fn parse(json: &str) -> Result<ProbeResult, MediaError> {
    let raw: RawProbe = serde_json::from_str(json).map_err(|e| MediaError::Parse(e.to_string()))?;
    let mut video = None;
    let mut audio = Vec::new();
    for stream in &raw.streams {
        match stream.codec_type.as_deref() {
            Some("video") if video.is_none() => {
                let (num, den) = fraction(stream.r_frame_rate.as_deref().unwrap_or("0/0"));
                video = Some(VideoStream {
                    index: stream.index,
                    codec: stream.codec_name.clone().unwrap_or_default(),
                    width: stream.width.unwrap_or(0),
                    height: stream.height.unwrap_or(0),
                    frame_rate_num: num,
                    frame_rate_den: den,
                    start_time_s: seconds(stream.start_time.as_deref()),
                });
            }
            Some("audio") => audio.push(AudioStream {
                index: stream.index,
                audio_position: audio.len() as u32,
                codec: stream.codec_name.clone().unwrap_or_default(),
                language: stream
                    .tags
                    .as_ref()
                    .and_then(|t| t.language.clone())
                    .filter(|l| l != "und"),
                channels: stream.channels.unwrap_or(0),
                sample_rate: stream
                    .sample_rate
                    .as_deref()
                    .and_then(|r| r.parse().ok())
                    .unwrap_or(0),
                start_time_s: seconds(stream.start_time.as_deref()),
            }),
            _ => {}
        }
    }
    let duration_s = raw
        .format
        .and_then(|f| f.duration)
        .and_then(|d| d.parse().ok())
        .unwrap_or(0.0);
    Ok(ProbeResult {
        duration_s,
        video,
        audio,
    })
}

/// The audio track to transcribe: the one tagged English, or the only one there is.
pub fn english_track(probe: &ProbeResult) -> Result<&AudioStream, MediaError> {
    let english = |s: &&AudioStream| matches!(s.language.as_deref(), Some("eng" | "en"));
    if let Some(track) = probe.audio.iter().find(english) {
        return Ok(track);
    }
    match probe.audio.as_slice() {
        [] => Err(MediaError::NoAudio),
        [only] => Ok(only),
        several => Err(MediaError::AmbiguousAudio(several.len())),
    }
}

fn fraction(text: &str) -> (u32, u32) {
    let mut parts = text.split('/');
    let num = parts.next().and_then(|n| n.parse().ok()).unwrap_or(0);
    let den = parts.next().and_then(|d| d.parse().ok()).unwrap_or(0);
    (num, den)
}

fn seconds(text: Option<&str>) -> f64 {
    text.and_then(|t| t.parse().ok()).unwrap_or(0.0)
}

#[derive(Deserialize)]
struct RawProbe {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
}

#[derive(Deserialize)]
struct RawStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    start_time: Option<String>,
    channels: Option<u32>,
    sample_rate: Option<String>,
    tags: Option<RawTags>,
}

#[derive(Deserialize)]
struct RawTags {
    language: Option<String>,
}

#[derive(Deserialize)]
struct RawFormat {
    duration: Option<String>,
}

#[cfg(test)]
#[path = "tests/probe.rs"]
mod tests;

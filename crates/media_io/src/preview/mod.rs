//! A short clip played back for review: FFmpeg sends its sound to the desktop's sound server
//! through the `pulse` output device and pipes small raw frames of its picture.
//!
//! **Role:** build the FFmpeg command lines that play a clip's sound (from the video's track, or
//! from a 16 kHz mono stem in the work directory) and decode its picture to RGBA frames of a
//! given size and rate.
//!
//! **Position:** called by the app's line review, which runs the commands through
//! `child_process` and draws the frames.
//!
//! **Signals and state:** none; the commands are only built here.
//!
//! **Invariants:** the video is only read; a clip never starts before the video or lasts less than
//! a tenth of a second.

use std::path::Path;

/// The clip to play: where it starts and how long it lasts, in video seconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clip {
    pub start_s: f64,
    pub duration_s: f64,
}

impl Clip {
    /// The span `from_s`..`to_s`, padded by `pad_s` on each side, never before 0 and at least a
    /// tenth of a second long.
    pub fn around(from_s: f64, to_s: f64, pad_s: f64) -> Clip {
        let start_s = (from_s - pad_s).max(0.0);
        Clip {
            start_s,
            duration_s: (to_s + pad_s - start_s).max(0.1),
        }
    }
}

/// The name the sound stream carries in the desktop's mixer.
pub const STREAM_NAME: &str = "TBD Subtitles clip";

fn seconds(value: f64) -> String {
    format!("{value:.3}")
}

/// FFmpeg arguments that play `clip` of audio track `audio_position` of `video` through `pulse`.
pub fn track_sound(video: &Path, audio_position: u32, clip: Clip) -> Vec<String> {
    vec![
        "-nostdin".into(),
        "-hide_banner".into(),
        "-v".into(),
        "error".into(),
        "-ss".into(),
        seconds(clip.start_s),
        "-t".into(),
        seconds(clip.duration_s),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-map".into(),
        format!("0:a:{audio_position}"),
        "-vn".into(),
        "-f".into(),
        "pulse".into(),
        STREAM_NAME.into(),
    ]
}

/// FFmpeg arguments that play `clip` of a raw 16 kHz mono `f32` stem through `pulse`.
pub fn stem_sound(stem: &Path, clip: Clip) -> Vec<String> {
    vec![
        "-nostdin".into(),
        "-hide_banner".into(),
        "-v".into(),
        "error".into(),
        "-f".into(),
        "f32le".into(),
        "-ar".into(),
        "16000".into(),
        "-ac".into(),
        "1".into(),
        "-ss".into(),
        seconds(clip.start_s),
        "-t".into(),
        seconds(clip.duration_s),
        "-i".into(),
        stem.to_string_lossy().into_owned(),
        "-f".into(),
        "pulse".into(),
        STREAM_NAME.into(),
    ]
}

/// The size of the preview frames for a video of `width` by `height`: `height` lines of the
/// given height and the width that keeps the shape, both even.
pub fn frame_size(width: u32, height: u32, lines: u32) -> (u32, u32) {
    let lines = lines.max(2) & !1;
    if width == 0 || height == 0 {
        return ((lines * 16 / 9) & !1, lines);
    }
    let across = (f64::from(width) * f64::from(lines) / f64::from(height)).round() as u32;
    (across.max(2) & !1, lines)
}

/// FFmpeg arguments that decode `clip` of `video` to raw RGBA frames of `size` at `fps` on stdout.
pub fn frames(video: &Path, clip: Clip, size: (u32, u32), fps: u32) -> Vec<String> {
    vec![
        "-nostdin".into(),
        "-hide_banner".into(),
        "-v".into(),
        "error".into(),
        "-ss".into(),
        seconds(clip.start_s),
        "-t".into(),
        seconds(clip.duration_s),
        "-i".into(),
        video.to_string_lossy().into_owned(),
        "-an".into(),
        "-vf".into(),
        format!("scale={}:{},fps={fps}", size.0, size.1),
        "-f".into(),
        "rawvideo".into(),
        "-pix_fmt".into(),
        "rgba".into(),
        "pipe:1".into(),
    ]
}

#[cfg(test)]
#[path = "tests/preview.rs"]
mod tests;

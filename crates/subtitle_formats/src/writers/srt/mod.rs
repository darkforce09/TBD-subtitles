//! SubRip (`.srt`): numbered cues, `HH:MM:SS,mmm` times, `<i>` italics.

use crate::cue::{CueTrack, FrameRate};

/// The whole file: cues numbered from 1, each followed by a blank line, LF line ends, UTF-8
/// (a Rust string). Times are the cue frames rounded to the nearest millisecond.
pub fn write(track: &CueTrack) -> String {
    let mut out = String::new();
    for (i, cue) in track.cues.iter().enumerate() {
        out.push_str(&format!(
            "{}\n{} --> {}\n",
            i + 1,
            timestamp(track.frame_rate, cue.start),
            timestamp(track.frame_rate, cue.end)
        ));
        for line in &cue.lines {
            if line.italic {
                out.push_str(&format!("<i>{}</i>\n", line.text));
            } else {
                out.push_str(&line.text);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out
}

/// `HH:MM:SS,mmm` for the start of `frame`.
pub fn timestamp(rate: FrameRate, frame: u64) -> String {
    let ms = rate.millis(frame);
    format!(
        "{:02}:{:02}:{:02},{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

#[cfg(test)]
#[path = "tests/srt.rs"]
mod tests;

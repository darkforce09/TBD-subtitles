//! WebVTT (`.vtt`): the `WEBVTT` header, `HH:MM:SS.mmm` times, `<i>` italics.

use crate::cue::{CueTrack, FrameRate};

/// The whole file: the `WEBVTT` header, then each cue as its timing line and its lines, each cue
/// followed by a blank line; LF line ends, UTF-8. Times are the cue frames rounded to the nearest
/// millisecond. `&`, `<` and `>` in the text are written as character references, as WebVTT
/// requires, so the text shows as given.
pub fn write(track: &CueTrack) -> String {
    let mut out = String::from("WEBVTT\n\n");
    for cue in &track.cues {
        out.push_str(&format!(
            "{} --> {}\n",
            timestamp(track.frame_rate, cue.start),
            timestamp(track.frame_rate, cue.end)
        ));
        for line in &cue.lines {
            let text = escape(&line.text);
            if line.italic {
                out.push_str(&format!("<i>{text}</i>\n"));
            } else {
                out.push_str(&text);
                out.push('\n');
            }
        }
        out.push('\n');
    }
    out
}

/// `HH:MM:SS.mmm` for the start of `frame`.
pub fn timestamp(rate: FrameRate, frame: u64) -> String {
    let ms = rate.millis(frame);
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        ms / 3_600_000,
        ms / 60_000 % 60,
        ms / 1000 % 60,
        ms % 1000
    )
}

/// Text with the three characters WebVTT reserves written as references.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
#[path = "tests/vtt.rs"]
mod tests;

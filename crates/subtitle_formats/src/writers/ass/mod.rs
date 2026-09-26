//! Advanced SubStation Alpha (`.ass`): a script header, one bottom-centred dialogue style, and
//! `H:MM:SS.cc` times.
//!
//! **Role:** write a cue track as an ASS script that players such as VLC show like the SRT file:
//! white text with a black outline at the bottom centre, `\N` between lines, `{\i1}` italics.
//!
//! **Position:** called by the output step when the owner's output format is ASS; uses
//! `crate::cue`.
//!
//! **Signals and state:** none; returns the file as a string.
//!
//! **Invariants:** text shows as given: `{` and `}` are escaped so no text opens an override
//! block, and a backslash before `n`, `N` or `h` is kept apart from the letter by a word joiner
//! (U+2060) so it is not read as a line break or a hard space.

use crate::cue::{CueTrack, FrameRate};

/// The script's canvas; font size and margins are in its pixels.
const PLAY_RES: (u32, u32) = (1920, 1080);

/// The whole file: `[Script Info]`, `[V4+ Styles]` with the `Default` style, and `[Events]` with
/// one `Dialogue` line per cue; LF line ends, UTF-8. Times are the cue frames rounded to the
/// nearest centisecond.
pub fn write(track: &CueTrack) -> String {
    let mut out = format!(
        "[Script Info]\n\
         ScriptType: v4.00+\n\
         PlayResX: {}\n\
         PlayResY: {}\n\
         WrapStyle: 2\n\
         ScaledBorderAndShadow: yes\n\
         \n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, \
         BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, \
         BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Default,Arial,64,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,\
         1,3,1,2,120,120,54,1\n\
         \n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
        PLAY_RES.0, PLAY_RES.1
    );
    for cue in &track.cues {
        let text: Vec<String> = cue
            .lines
            .iter()
            .map(|line| {
                let text = escape(&line.text);
                if line.italic {
                    format!("{{\\i1}}{text}{{\\i0}}")
                } else {
                    text
                }
            })
            .collect();
        out.push_str(&format!(
            "Dialogue: 0,{},{},Default,,0,0,0,,{}\n",
            timestamp(track.frame_rate, cue.start),
            timestamp(track.frame_rate, cue.end),
            text.join("\\N")
        ));
    }
    out
}

/// `H:MM:SS.cc` for the start of `frame`.
pub fn timestamp(rate: FrameRate, frame: u64) -> String {
    let cs = (rate.millis(frame) + 5) / 10;
    format!(
        "{}:{:02}:{:02}.{:02}",
        cs / 360_000,
        cs / 6000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}

/// Text that ASS shows as given.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '\\' if matches!(chars.peek(), Some('n' | 'N' | 'h')) => out.push_str("\\\u{2060}"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/ass.rs"]
mod tests;

use super::*;
use crate::cue::{Cue, CueKind, CueLine};

fn track(cues: Vec<Cue>) -> CueTrack {
    CueTrack {
        frame_rate: FrameRate::FILM,
        cues,
    }
}

#[test]
fn timestamps_use_a_dot_before_the_milliseconds() {
    let rate = FrameRate::FILM;
    assert_eq!(timestamp(rate, 0), "00:00:00.000");
    assert_eq!(timestamp(rate, 1), "00:00:00.042");
    assert_eq!(timestamp(rate, 24 * 3725), "01:02:05.000");
}

#[test]
fn the_file_starts_with_the_header_and_cues_are_unnumbered() {
    let text = write(&track(vec![Cue {
        start: 24,
        end: 72,
        lines: vec![
            CueLine::plain("-Are you coming?"),
            CueLine::italic("-In a minute."),
        ],
        kind: CueKind::Dialogue,
    }]));
    assert_eq!(
        text,
        "WEBVTT\n\n00:00:01.000 --> 00:00:03.000\n-Are you coming?\n<i>-In a minute.</i>\n\n"
    );
}

#[test]
fn reserved_characters_are_written_as_references() {
    let text = write(&track(vec![Cue {
        start: 0,
        end: 30,
        lines: vec![CueLine::plain("Law & Luffy <3 -> Señor Pink…")],
        kind: CueKind::Dialogue,
    }]));
    assert!(
        text.contains("Law &amp; Luffy &lt;3 -&gt; Señor Pink…\n"),
        "{text}"
    );
}

#[test]
fn an_empty_track_is_the_header_alone() {
    assert_eq!(write(&track(vec![])), "WEBVTT\n\n");
}

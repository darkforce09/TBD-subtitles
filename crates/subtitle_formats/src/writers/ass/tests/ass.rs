use super::*;
use crate::cue::{Cue, CueKind, CueLine};

fn track(cues: Vec<Cue>) -> CueTrack {
    CueTrack {
        frame_rate: FrameRate::FILM,
        cues,
    }
}

#[test]
fn timestamps_use_hours_and_rounded_centiseconds() {
    let rate = FrameRate::FILM;
    assert_eq!(timestamp(rate, 0), "0:00:00.00");
    assert_eq!(timestamp(rate, 1), "0:00:00.04");
    assert_eq!(timestamp(rate, 24 * 61 + 12), "0:01:01.50");
    assert_eq!(timestamp(rate, 24 * 3725), "1:02:05.00");
}

#[test]
fn every_cue_is_one_dialogue_line_in_the_default_style() {
    let text = write(&track(vec![
        Cue {
            start: 24,
            end: 72,
            lines: vec![
                CueLine::plain("-Are you coming?"),
                CueLine::plain("-In a minute."),
            ],
            kind: CueKind::Dialogue,
        },
        Cue {
            start: 80,
            end: 110,
            lines: vec![CueLine::italic("Meanwhile,")],
            kind: CueKind::Dialogue,
        },
    ]));
    assert!(text.starts_with("[Script Info]\nScriptType: v4.00+\n"));
    assert!(text.contains("\nStyle: Default,Arial,64,"));
    assert!(text.ends_with(
        "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,-Are you coming?\\N-In a minute.\n\
         Dialogue: 0,0:00:03.33,0:00:04.58,Default,,0,0,0,,{\\i1}Meanwhile,{\\i0}\n"
    ));
}

#[test]
fn braces_and_backslash_sequences_show_as_written() {
    let text = write(&track(vec![Cue {
        start: 0,
        end: 30,
        lines: vec![CueLine::plain("{sic} C:\\new Señor Pink…")],
        kind: CueKind::Dialogue,
    }]));
    assert!(
        text.contains(",,\\{sic\\} C:\\\u{2060}new Señor Pink…\n"),
        "{text}"
    );
}

#[test]
fn an_empty_track_is_the_header_alone() {
    let text = write(&track(vec![]));
    assert!(text.ends_with(
        "Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n"
    ));
    assert!(!text.contains("Dialogue:"));
}

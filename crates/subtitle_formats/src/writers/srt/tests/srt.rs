use super::*;
use crate::cue::{Cue, CueKind, CueLine};

fn track(cues: Vec<Cue>) -> CueTrack {
    CueTrack {
        frame_rate: FrameRate::FILM,
        cues,
    }
}

#[test]
fn timestamps_use_hours_minutes_seconds_and_rounded_milliseconds() {
    let rate = FrameRate::FILM;
    assert_eq!(timestamp(rate, 0), "00:00:00,000");
    assert_eq!(timestamp(rate, 1), "00:00:00,042");
    assert_eq!(timestamp(rate, 24 * 61 + 12), "00:01:01,500");
    assert_eq!(timestamp(rate, 24 * 3725), "01:02:05,000");
}

#[test]
fn cues_are_numbered_from_one_with_blank_lines_between() {
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
            lines: vec![CueLine::plain("[explosion]")],
            kind: CueKind::Sound,
        },
    ]));
    assert_eq!(
        text,
        "1\n00:00:01,000 --> 00:00:03,000\n-Are you coming?\n-In a minute.\n\n\
         2\n00:00:03,333 --> 00:00:04,583\n[explosion]\n\n"
    );
}

#[test]
fn italic_lines_are_wrapped_one_by_one() {
    let text = write(&track(vec![Cue {
        start: 0,
        end: 48,
        lines: vec![
            CueLine::italic("Meanwhile, at the palace,"),
            CueLine::plain("[door opens]"),
        ],
        kind: CueKind::Dialogue,
    }]));
    assert!(text.contains("<i>Meanwhile, at the palace,</i>\n[door opens]\n"));
}

#[test]
fn an_empty_track_is_an_empty_file() {
    assert_eq!(write(&track(vec![])), "");
}

#[test]
fn the_writer_keeps_text_as_given() {
    let text = write(&track(vec![Cue {
        start: 0,
        end: 30,
        lines: vec![CueLine::plain("Señor Pink… ♪")],
        kind: CueKind::Dialogue,
    }]));
    assert!(text.contains("Señor Pink… ♪\n"));
}

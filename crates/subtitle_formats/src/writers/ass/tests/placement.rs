use super::*;
use crate::cue::{CueKind, CueLine, CueTrack};
use crate::writers::ass::{write, write_with};

/// "Hello there" from 1 s to 3 s: its bottom box is [777, 946, 1143, 1026] and its top box
/// [777, 54, 1143, 134], to the pixel after rounding.
fn cue() -> Cue {
    Cue {
        start: 24,
        end: 72,
        lines: vec![CueLine::plain("Hello there")],
        kind: CueKind::Dialogue,
    }
}

fn track() -> CueTrack {
    CueTrack {
        frame_rate: FrameRate::FILM,
        cues: vec![cue()],
    }
}

fn obstacle(start_s: f64, end_s: f64, rect: [f64; 4]) -> Obstacle {
    Obstacle {
        start_s,
        end_s,
        rect,
    }
}

const LOWER_THIRD: [f64; 4] = [700.0, 900.0, 1200.0, 1060.0];
const TOP_TITLE: [f64; 4] = [700.0, 40.0, 1200.0, 150.0];

#[test]
fn the_box_sits_between_the_margins_one_line_height_per_line() {
    let [l, t, r, b] = cue_box(&cue(), Band::Bottom);
    assert_eq!(
        [l.round(), t.round(), r.round(), b.round()],
        [777.0, 946.0, 1143.0, 1026.0]
    );
    let [l, t, r, b] = cue_box(&cue(), Band::Top);
    assert_eq!(
        [l.round(), t.round(), r.round(), b.round()],
        [777.0, 54.0, 1143.0, 134.0]
    );
    let long = Cue {
        lines: vec![CueLine::plain("x".repeat(80)), CueLine::plain("y")],
        ..cue()
    };
    assert_eq!(
        cue_box(&long, Band::Bottom),
        [120.0, 866.0, 1800.0, 1026.0],
        "as wide as the margins allow, two lines high"
    );
}

#[test]
fn a_cue_over_lettered_writing_moves_to_the_top() {
    let blocked = [obstacle(0.5, 2.0, LOWER_THIRD)];
    assert_eq!(band(&cue(), FrameRate::FILM, &blocked), Band::Top);
    let text = write_with(&track(), &blocked);
    assert!(
        text.ends_with("Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,{\\an8}Hello there\n"),
        "{text}"
    );
}

#[test]
fn writing_at_another_time_or_at_the_top_leaves_the_cue_at_the_bottom() {
    for obstacles in [
        vec![obstacle(3.0, 6.0, LOWER_THIRD)],
        vec![obstacle(0.0, 1.0, LOWER_THIRD)],
        vec![obstacle(0.0, 5.0, TOP_TITLE)],
        vec![obstacle(0.0, 5.0, [0.0, 900.0, 300.0, 1080.0])],
    ] {
        assert_eq!(
            band(&cue(), FrameRate::FILM, &obstacles),
            Band::Bottom,
            "{obstacles:?}"
        );
        assert_eq!(write_with(&track(), &obstacles), write(&track()));
    }
}

#[test]
fn with_both_bands_blocked_the_cue_takes_the_side_that_covers_less() {
    let small_bottom = [800.0, 1000.0, 820.0, 1020.0];
    let small_top = [800.0, 60.0, 820.0, 80.0];
    assert_eq!(
        band(
            &cue(),
            FrameRate::FILM,
            &[
                obstacle(0.0, 5.0, small_bottom),
                obstacle(0.0, 5.0, TOP_TITLE)
            ]
        ),
        Band::Bottom
    );
    assert_eq!(
        band(
            &cue(),
            FrameRate::FILM,
            &[
                obstacle(0.0, 5.0, LOWER_THIRD),
                obstacle(0.0, 5.0, small_top)
            ]
        ),
        Band::Top
    );
    assert_eq!(
        band(
            &cue(),
            FrameRate::FILM,
            &[
                obstacle(0.0, 5.0, small_bottom),
                obstacle(0.0, 5.0, small_top)
            ]
        ),
        Band::Bottom,
        "the bottom wins a tie"
    );
}

#[test]
fn the_normal_file_has_no_placement_override() {
    let text = write(&track());
    assert!(!text.contains("\\an8"), "{text}");
    assert!(text.ends_with(",Default,,0,0,0,,Hello there\n"), "{text}");
}

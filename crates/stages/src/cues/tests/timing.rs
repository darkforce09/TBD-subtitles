use subtitle_formats::cue::{CueKind, CueLine, FrameRate};

use super::*;

fn rules() -> FrameRules {
    FrameRules::new(FrameRate::FILM, 3600.0)
}

fn draft(text: &str, start_s: f64, end_s: f64) -> Draft {
    Draft::new(
        vec![CueLine::plain(text)],
        CueKind::Dialogue,
        start_s,
        end_s,
    )
}

#[test]
fn the_rules_in_frames_at_24_fps() {
    let r = rules();
    assert_eq!(
        (
            r.lead_in,
            r.lead_out,
            r.min,
            r.max,
            r.gap,
            r.chain_max,
            r.shot_window
        ),
        (1, 12, 20, 168, 2, 11, 12)
    );
    assert_eq!(r.reading_frames(40), 48);
}

#[test]
fn a_cue_starts_one_frame_early_and_ends_half_a_second_late() {
    let mut d = [draft("Hello there, friend.", 10.0, 11.5)];
    initial(&mut d, &rules());
    assert_eq!((d[0].start, d[0].end), (239, 276 + 12));
}

#[test]
fn short_and_fast_cues_grow_into_the_gap_but_not_past_the_next_cue() {
    let r = rules();
    let mut d = [
        draft("Hi!", 10.0, 10.1),
        draft("This line is long enough to need time.", 10.5, 10.9),
    ];
    initial(&mut d, &r);
    extend(&mut d, &r);
    separate(&mut d, &r);
    assert!(d[0].end + r.gap <= d[1].start, "{d:?}");
    assert!(
        d[1].end - d[1].start >= r.reading_frames(d[1].chars()),
        "{d:?}"
    );
    assert!(d[1].end - d[1].start >= r.min);
}

#[test]
fn a_cue_over_seven_seconds_is_cut_back_but_keeps_its_speech() {
    let r = rules();
    let mut d = [draft("x", 0.0, 7.5)];
    initial(&mut d, &r);
    extend(&mut d, &r);
    assert_eq!(d[0].end, r.rate.frame_ceil(7.5));
}

#[test]
fn overlapping_cues_are_pulled_apart_keeping_speech_first() {
    let r = rules();
    let mut d = [draft("a", 1.0, 3.0), draft("b", 2.9, 4.0)];
    initial(&mut d, &r);
    separate(&mut d, &r);
    assert_eq!(d[0].end, 72);
    assert_eq!(d[1].start, 74);
}

#[test]
fn gaps_of_three_to_eleven_frames_close_to_two() {
    let r = rules();
    let mut d = [
        draft("a", 0.0, 1.0),
        draft("b", 0.0, 1.0),
        draft("c", 0.0, 1.0),
    ];
    (d[0].start, d[0].end) = (0, 30);
    (d[1].start, d[1].end) = (40, 70);
    (d[2].start, d[2].end) = (82, 100);
    chain(&mut d, &r);
    assert_eq!(d[0].end, 38);
    assert_eq!(d[1].end, 70, "a 12-frame gap stays");
}

#[test]
fn no_cue_ends_after_the_video() {
    let r = FrameRules::new(FrameRate::FILM, 10.0);
    let mut d = [draft("a", 9.8, 9.9)];
    initial(&mut d, &r);
    clamp(&mut d, &r);
    assert_eq!(d[0].end, 240);
    assert!(d[0].start < d[0].end);
}

#[test]
fn a_cramped_cue_takes_back_the_lead_out_of_the_cue_before() {
    let r = rules();
    // "What is his deal?" then "I don't know." 0.64 s later, then the next line 0.17 s after.
    let mut d = [
        draft("What is his deal?", 245.116, 245.916),
        draft("I don't know.", 246.556, 247.036),
        draft("Said something about Straw Hat.", 247.3, 249.0),
    ];
    initial(&mut d, &r);
    for _ in 0..2 {
        extend(&mut d, &r);
        separate(&mut d, &r);
    }
    assert!(d[1].end - d[1].start >= r.min, "{d:?}");
    assert!(
        d[0].end + r.gap <= d[1].start && d[1].end + r.gap <= d[2].start,
        "{d:?}"
    );
    assert!(
        d[0].end >= r.rate.frame_ceil(245.916),
        "the question keeps its speech: {d:?}"
    );
    assert!(d[0].end - d[0].start >= r.min, "{d:?}");
}

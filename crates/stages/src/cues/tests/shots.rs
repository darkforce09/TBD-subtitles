use job_model::outputs::ShotCut;
use subtitle_formats::cue::{CueKind, CueLine};

use super::*;

fn rules() -> FrameRules {
    FrameRules::new(FrameRate::FILM, 3600.0)
}

fn shots(cuts: &[(f64, f64)]) -> ShotChanges {
    ShotChanges {
        cuts: cuts
            .iter()
            .map(|&(time_s, score)| ShotCut { time_s, score })
            .collect(),
    }
}

fn timed(start: u64, end: u64, speech: (f64, f64)) -> Draft {
    let mut d = Draft::new(
        vec![CueLine::plain("x")],
        CueKind::Dialogue,
        speech.0,
        speech.1,
    );
    (d.start, d.end) = (start, end);
    d
}

#[test]
fn weak_changes_are_ignored_and_close_ones_merge_into_the_strongest() {
    let found = cut_frames(
        &shots(&[
            (1.0, 12.0),
            (2.0, 25.0),
            (2.2, 40.0),
            (2.4, 30.0),
            (5.0, 20.0),
        ]),
        20.0,
        FrameRate::FILM,
    );
    assert_eq!(found, vec![53, 120]);
}

#[test]
fn a_cue_starts_on_a_cut_its_speech_follows_closely() {
    let r = rules();
    // Speech at frame 110, cut at 100: within 12 frames.
    let mut d = [timed(109, 150, (110.0 / 24.0, 130.0 / 24.0))];
    snap(&mut d, &[100], &r);
    assert_eq!(d[0].start, 100);
    // Cut 20 frames before: too far.
    let mut far = [timed(109, 150, (110.0 / 24.0, 130.0 / 24.0))];
    snap(&mut far, &[90], &r);
    assert_eq!(far[0].start, 109);
}

#[test]
fn a_cue_ends_two_frames_before_a_nearby_cut_unless_speech_runs_past_it() {
    let r = rules();
    let mut d = [timed(100, 150, (100.0 / 24.0, 130.0 / 24.0))];
    snap(&mut d, &[155], &r);
    assert_eq!(d[0].end, 153);
    // Speech runs to frame 150; the cut at 145 is inside the speech: clear it by 12 frames.
    let mut past = [timed(100, 152, (100.0 / 24.0, 150.0 / 24.0))];
    snap(&mut past, &[145], &r);
    assert_eq!(past[0].end, 157);
}

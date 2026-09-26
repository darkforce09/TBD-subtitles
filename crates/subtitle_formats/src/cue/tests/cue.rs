use super::*;

#[test]
fn a_zero_rate_is_refused() {
    assert!(FrameRate::new(0, 1).is_none());
    assert!(FrameRate::new(24, 0).is_none());
    assert_eq!(FrameRate::new(24, 1), Some(FrameRate::FILM));
}

#[test]
fn film_frames_round_to_milliseconds() {
    let rate = FrameRate::FILM;
    assert_eq!(rate.millis(0), 0);
    assert_eq!(rate.millis(1), 42);
    assert_eq!(rate.millis(2), 83);
    assert_eq!(rate.millis(24), 1000);
    assert_eq!(rate.millis(24 * 3600), 3_600_000);
}

#[test]
fn ntsc_frames_round_to_milliseconds() {
    let rate = FrameRate::new(24000, 1001).expect("rate");
    assert_eq!(rate.millis(24), 1001);
    assert_eq!(rate.millis(1), 42);
}

#[test]
fn seconds_snap_to_frame_boundaries() {
    let rate = FrameRate::FILM;
    assert_eq!(rate.frame_floor(1.0), 24);
    assert_eq!(rate.frame_ceil(1.0), 24);
    assert_eq!(rate.frame_floor(1.03), 24);
    assert_eq!(rate.frame_ceil(1.03), 25);
    assert_eq!(rate.frame_round(1.03), 25);
    assert_eq!(rate.frame_floor(-0.5), 0);
    // A time computed from a frame snaps back onto the same frame.
    assert_eq!(rate.frame_floor(rate.seconds(1234)), 1234);
    assert_eq!(rate.frame_ceil(rate.seconds(1234)), 1234);
}

#[test]
fn reading_speed_counts_every_character_but_line_breaks() {
    let cue = Cue {
        start: 0,
        end: 48,
        lines: vec![
            CueLine::plain("-Are you coming?"),
            CueLine::plain("-In a minute."),
        ],
        kind: CueKind::Dialogue,
    };
    assert_eq!(cue.chars(), 29);
    assert!((cue.cps(FrameRate::FILM) - 14.5).abs() < 1e-9);
    assert_eq!(cue.text(), "-Are you coming? -In a minute.");
    let empty = Cue {
        start: 5,
        end: 5,
        lines: vec![],
        kind: CueKind::Sound,
    };
    assert_eq!(empty.cps(FrameRate::FILM), 0.0);
}

#[test]
fn a_track_round_trips_through_json() {
    let track = CueTrack {
        frame_rate: FrameRate::new(24000, 1001).expect("rate"),
        cues: vec![Cue {
            start: 3,
            end: 40,
            lines: vec![CueLine::italic("Long ago…")],
            kind: CueKind::Music,
        }],
    };
    let json = serde_json::to_string(&track).expect("json");
    assert!(json.contains("\"kind\":\"music\""));
    let back: CueTrack = serde_json::from_str(&json).expect("parse");
    assert_eq!(back, track);
}

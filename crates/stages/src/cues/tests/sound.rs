use subtitle_formats::cue::FrameRate;

use super::*;

fn rules() -> FrameRules {
    FrameRules::new(FrameRate::FILM, 3600.0)
}

fn dialogue(start: u64, end: u64) -> Draft {
    let mut d = Draft::new(
        vec![CueLine::plain("Look out!")],
        CueKind::Dialogue,
        start as f64 / 24.0,
        end as f64 / 24.0,
    );
    (d.start, d.end) = (start, end);
    d
}

fn sound(text: &str, start_s: f64, end_s: f64, kind: CandidateKind) -> SoundCue {
    SoundCue {
        candidate: "S001".into(),
        kind,
        start_s,
        end_s,
        text: text.into(),
    }
}

#[test]
fn a_sound_in_a_free_stretch_gets_its_own_cue() {
    let mut drafts = vec![dialogue(0, 48), dialogue(240, 300)];
    let dropped = place(
        &mut drafts,
        &[sound("[explosion]", 4.0, 5.0, CandidateKind::Effect)],
        &rules(),
    );
    assert!(dropped.is_empty());
    assert_eq!(drafts.len(), 3);
    assert_eq!(drafts[1].kind, CueKind::Sound);
    assert_eq!(drafts[1].start, 96);
    assert!(drafts[1].end - drafts[1].start >= 24 && drafts[1].end + 2 <= 240);
}

#[test]
fn a_sound_under_a_one_line_cue_becomes_its_second_line() {
    let mut drafts = vec![dialogue(0, 100), dialogue(102, 200)];
    let dropped = place(
        &mut drafts,
        &[sound("[gasps]", 2.0, 2.5, CandidateKind::Voice)],
        &rules(),
    );
    assert!(dropped.is_empty());
    assert_eq!(drafts.len(), 2);
    assert_eq!(drafts[0].lines.len(), 2);
    assert!(drafts[0].lines.iter().all(|l| !l.italic));
}

#[test]
fn a_sound_with_no_room_is_dropped() {
    let mut full = dialogue(0, 100);
    full.lines.push(CueLine::plain("second line"));
    let mut drafts = vec![full, dialogue(102, 200)];
    let dropped = place(
        &mut drafts,
        &[sound("[gasps]", 2.0, 2.5, CandidateKind::Voice)],
        &rules(),
    );
    assert_eq!(dropped, vec!["2.0s [gasps]".to_string()]);
}

#[test]
fn a_song_gets_one_music_cue_of_at_most_seven_seconds() {
    let mut drafts = vec![dialogue(24 * 200, 24 * 202)];
    place(
        &mut drafts,
        &[sound(
            "[upbeat music playing]",
            0.0,
            148.0,
            CandidateKind::Song,
        )],
        &rules(),
    );
    assert_eq!(drafts[0].kind, CueKind::Music);
    assert_eq!((drafts[0].start, drafts[0].end), (0, 168));
}

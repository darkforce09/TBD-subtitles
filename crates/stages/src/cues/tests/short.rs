use subtitle_formats::cue::FrameRate;

use super::*;

fn rules() -> FrameRules {
    FrameRules::new(FrameRate::FILM, 3600.0)
}

/// A timed dialogue cue: lines, frames, and speech in seconds.
fn cue(lines: &[&str], frames: (u64, u64), speech: (f64, f64)) -> Draft {
    let mut d = Draft::new(
        lines.iter().map(|l| CueLine::plain(*l)).collect(),
        CueKind::Dialogue,
        speech.0,
        speech.1,
    );
    (d.start, d.end) = frames;
    d
}

fn texts(d: &Draft) -> Vec<&str> {
    d.lines.iter().map(|l| l.text.as_str()).collect()
}

#[test]
fn a_short_cue_joins_the_cue_before_when_the_words_fit() {
    let mut drafts = vec![
        cue(&["Alright then, Trafalgar."], (868, 905), (36.2, 37.2)),
        cue(&["Ah!"], (907, 926), (37.8, 38.0)),
        cue(&["Law!"], (928, 953), (38.7, 39.0)),
    ];
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 2);
    assert_eq!(texts(&drafts[0]), ["Alright then, Trafalgar. Ah!"]);
    assert_eq!((drafts[0].start, drafts[0].end), (868, 926));
}

#[test]
fn a_marked_speaker_change_shares_with_dashes() {
    let mut drafts = vec![
        cue(&["with this."], (647, 667), (27.0, 27.4)),
        cue(&["Huh?"], (669, 683), (27.9, 28.1)),
        cue(&["Let me introduce the team."], (685, 738), (28.6, 30.0)),
    ];
    drafts[1].starts_speaker = true;
    drafts[2].starts_speaker = true;
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 2);
    assert_eq!(texts(&drafts[0]), ["-with this.", "-Huh?"]);
    assert_eq!(texts(&drafts[1]), ["Let me introduce the team."]);
}

#[test]
fn full_neighbours_leave_the_cue_after() {
    let mut drafts = vec![
        cue(
            &[
                "I bet you wouldn't have stumbled if you",
                "still had that extra leg right about now,",
            ],
            (150, 247),
            (6.3, 9.8),
        ),
        cue(&["ha!"], (249, 261), (10.4, 10.45)),
        cue(&["You can't do this to him!"], (263, 310), (11.0, 12.4)),
    ];
    drafts[2].starts_speaker = true;
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 2);
    assert_eq!(texts(&drafts[1]), ["-ha!", "-You can't do this to him!"]);
    assert_eq!((drafts[1].start, drafts[1].end), (249, 310));
}

#[test]
fn with_no_neighbour_to_share_the_cue_grows_into_a_lead_out() {
    let r = rules();
    // The cue before is full but holds a lead-out past its speech; the cue after is a
    // two-speaker cue.
    let mut drafts = vec![
        cue(
            &[
                "We kind of hit it off while we were all",
                "staying at the palace and decided to go.",
            ],
            (100, 200),
            (4.2, 7.9),
        ),
        cue(&["Now!"], (202, 212), (8.45, 8.6)),
        cue(&["-Wait.", "-No."], (214, 250), (9.0, 10.0)),
    ];
    resolve(&mut drafts, &r);
    assert_eq!(drafts.len(), 3);
    let d = &drafts[1];
    assert_eq!(d.end - d.start, r.min, "{d:?}");
    assert!(drafts[0].end + r.gap <= d.start);
    let (onset, _) = d.speech_frames(r.rate);
    assert!(d.start <= onset);
}

#[test]
fn a_cue_of_the_minimum_is_left_alone() {
    let r = rules();
    let before = vec![
        cue(&["One."], (0, 20), (0.1, 0.4)),
        cue(&["Two."], (22, 42), (1.0, 1.3)),
    ];
    let mut drafts = before.clone();
    resolve(&mut drafts, &r);
    assert_eq!(drafts, before);
}

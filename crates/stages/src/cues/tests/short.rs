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
    // two-speaker cue that starts a new speaker.
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
    drafts[2].starts_speaker = true;
    resolve(&mut drafts, &r);
    assert_eq!(drafts.len(), 3);
    let d = &drafts[1];
    assert_eq!(d.end - d.start, r.min, "{d:?}");
    assert!(drafts[0].end + r.gap <= d.start);
    let (onset, _) = d.speech_frames(r.rate);
    assert!(d.start <= onset);
}

/// A full two-line cue, a short `Oh?` that starts a new speaker, and a dashed cue whose first
/// speaker says `first`.
fn before_a_dashed_cue(first: &str, next_starts_speaker: bool) -> Vec<Draft> {
    let mut drafts = vec![
        cue(
            &[
                "I'd like to see what you're wearing",
                "under that coat, if you don't mind.",
            ],
            (27692, 27744),
            (1153.9, 1156.0),
        ),
        cue(&["Oh?"], (27746, 27763), (1156.1, 1156.3)),
        cue(
            &[first, "-You say you'd like to see them?"],
            (27765, 27822),
            (1156.9, 1158.8),
        ),
    ];
    drafts[1].starts_speaker = true;
    drafts[2].starts_speaker = next_starts_speaker;
    drafts
}

#[test]
fn a_short_cue_opens_the_first_line_of_the_dashed_cue_it_continues() {
    let mut drafts = before_a_dashed_cue("-Panties?", false);
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 2);
    let d = &drafts[1];
    assert_eq!(
        texts(d),
        ["-Oh? Panties?", "-You say you'd like to see them?"]
    );
    assert_eq!((d.start, d.end), (27746, 27822));
    assert!(d.starts_speaker);
    assert_eq!((d.speech_start_s, d.speech_end_s), (1156.1, 1158.8));
}

#[test]
fn a_dashed_cue_starting_a_new_speaker_takes_no_short_cue() {
    let mut drafts = before_a_dashed_cue("-Panties?", true);
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 3);
    assert_eq!(texts(&drafts[1]), ["Oh?"]);
    assert_eq!(texts(&drafts[2])[0], "-Panties?");
}

#[test]
fn a_joined_line_of_42_characters_is_not_joined() {
    let first = "-Panties? The ones you wore yesterday?";
    assert_eq!(format!("-Oh? {}", &first[1..]).chars().count(), MAX_LINE);
    let mut drafts = before_a_dashed_cue(first, false);
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 3);
    assert_eq!(texts(&drafts[1]), ["Oh?"]);
    assert_eq!(texts(&drafts[2])[0], first);
}

#[test]
fn a_short_cue_closes_the_second_line_of_the_dashed_cue_it_continues() {
    let mut drafts = vec![
        cue(
            &["-Is that him?", "-I'm not sure,"],
            (1000, 1060),
            (41.7, 43.9),
        ),
        cue(&["but maybe."], (1062, 1070), (44.3, 44.5)),
    ];
    resolve(&mut drafts, &rules());
    assert_eq!(drafts.len(), 1);
    let d = &drafts[0];
    assert_eq!(texts(d), ["-Is that him?", "-I'm not sure, but maybe."]);
    assert_eq!((d.start, d.end), (1000, 1070));
    assert_eq!((d.speech_start_s, d.speech_end_s), (41.7, 44.5));
}

/// A cue before at `prev` frames with speech `prev_speech` in seconds, a one-frame `Hm?` at 6160,
/// and a dashed cue starting a new speaker at 6163.
fn around_a_one_frame_cue(prev: (u64, u64), prev_speech: (f64, f64)) -> Vec<Draft> {
    let mut drafts = vec![
        cue(
            &["We should get going before they notice."],
            prev,
            prev_speech,
        ),
        cue(&["Hm?"], (6160, 6161), (256.68, 256.7)),
        cue(&["-Hey!", "-What?"], (6163, 6200), (256.8, 258.2)),
    ];
    drafts[2].starts_speaker = true;
    drafts
}

#[test]
fn a_cue_short_after_growing_starts_earlier_into_free_time() {
    let r = rules();
    let mut drafts = around_a_one_frame_cue((6030, 6083), (251.3, 253.4));
    let before = drafts[0].clone();
    resolve(&mut drafts, &r);
    assert_eq!(drafts.len(), 3);
    assert_eq!(drafts[0], before);
    let d = &drafts[1];
    assert_eq!((d.start, d.end), (6141, 6161));
    assert_eq!(d.end - d.start, r.min);
}

#[test]
fn starting_earlier_stops_at_the_gap_after_the_cue_before() {
    let r = rules();
    // The cue before keeps no lead-out to give back, and with the short cue it passes 7 s.
    let mut drafts = around_a_one_frame_cue((5980, 6140), (249.2, 255.8));
    let before = drafts[0].clone();
    resolve(&mut drafts, &r);
    assert_eq!(drafts.len(), 3);
    assert_eq!(drafts[0], before);
    let d = &drafts[1];
    assert_eq!((d.start, d.end), (before.end + r.gap, 6161));
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

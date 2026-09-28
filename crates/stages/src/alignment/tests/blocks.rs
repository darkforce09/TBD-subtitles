use super::*;

pub(crate) fn kept_at(
    id: &str,
    sheet_index: usize,
    start_s: f64,
    end_s: f64,
    words: &[&str],
) -> Kept {
    let step = (end_s - start_s) / words.len().max(1) as f64;
    Kept {
        id: id.into(),
        sheet_index,
        start_s,
        end_s,
        words: words.iter().map(|w| w.to_string()).collect(),
        speaker_starts: vec![],
        new_speaker: false,
        narrator: false,
        unsure: false,
        backbone: words
            .iter()
            .enumerate()
            .map(|(i, w)| TimedWord {
                text: w.to_string(),
                start_s: start_s + i as f64 * step,
                end_s: start_s + (i + 1) as f64 * step,
                confidence: None,
            })
            .collect(),
    }
}

fn line(id: &str, t: &str, f: &[&str]) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
    }
}

fn utterance(id: &str) -> Utterance {
    Utterance {
        id: id.into(),
        start_s: 0.0,
        end_s: 1.0,
        words: vec![],
        locked: vec![],
        line: String::new(),
        hypotheses: vec![],
    }
}

#[test]
fn speaker_changes_become_indices() {
    assert_eq!(
        displayed_words("Are you coming? || In a minute."),
        (
            vec!["Are", "you", "coming?", "In", "a", "minute."]
                .into_iter()
                .map(String::from)
                .collect(),
            vec![3]
        )
    );
    assert_eq!(displayed_words("Hey!||No!").1, vec![1]);
    assert_eq!(displayed_words("|| Hey! ||").1, Vec::<usize>::new());
}

#[test]
fn lyric_dropped_and_empty_lines_are_not_kept() {
    let sheet = [
        utterance("U1"),
        utterance("U2"),
        utterance("U3"),
        utterance("U4"),
        utterance("U5"),
    ];
    let lines = [
        line("U1", "Hello.", &["NARR"]),
        line("U2", "la la", &["LYRIC"]),
        line("U3", "noise", &["DROP"]),
        line("U4", "  ", &[]),
        line("U5", "Wait! || What?", &["UNSURE"]),
    ];
    let kept = kept(&sheet, &lines, &[]);
    assert_eq!(
        kept.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
        vec!["U1", "U5"]
    );
    assert!(kept[0].narrator);
    assert!(kept[1].unsure);
    assert_eq!(kept[1].sheet_index, 4);
    assert_eq!(kept[1].speaker_starts, vec![1]);
}

/// An utterance whose backbone words are `words`, from `start_s` to `end_s`.
fn spoken(id: &str, start_s: f64, end_s: f64, words: &[&str]) -> Utterance {
    Utterance {
        words: kept_at(id, 0, start_s, end_s, words).backbone,
        start_s,
        end_s,
        ..utterance(id)
    }
}

fn window_of(sheet: &[Utterance], text: &str, spans: &[(f64, f64)]) -> (f64, f64) {
    let lines = [line(&sheet[0].id, text, &[])];
    let kept = kept(sheet, &lines, spans);
    (kept[0].start_s, kept[0].end_s)
}

#[test]
fn a_line_only_another_engine_heard_the_start_of_is_aligned_where_it_heard_it() {
    // Dressrosa 12: Parakeet heard only the answer; Whisper heard the heckle before it.
    let sheet = [spoken(
        "U0061",
        256.68,
        258.52,
        &["Shut", "your", "filthy", "mouths!"],
    )];
    let text = "Yeah, that's right! We want to see her suffer! || Shut your filthy mouths!";
    assert_eq!(
        window_of(&sheet, text, &[(254.12, 258.52)]),
        (254.12, 258.52)
    );
}

#[test]
fn a_line_the_backbone_covers_keeps_the_backbone_window() {
    let sheet = [spoken(
        "U1",
        10.0,
        11.5,
        &["Law,", "the", "Birdcage", "closes."],
    )];
    // Another engine heard a word earlier and a word later, but the line shows neither.
    let spans = [(8.0, 13.0)];
    assert_eq!(
        window_of(&sheet, "Law, the bird cage closes.", &spans),
        (10.0, 11.5)
    );
    assert_eq!(
        window_of(&sheet, "Law, the Birdcage closes.", &spans),
        (10.0, 11.5)
    );
}

#[test]
fn without_heard_spans_every_window_is_the_backbone_window() {
    let sheet = [spoken("U1", 5.0, 6.0, &["closing"])];
    assert_eq!(window_of(&sheet, "It's closing in!", &[]), (5.0, 6.0));
}

#[test]
fn a_widened_window_stops_at_the_next_kept_window() {
    // Dressrosa 12: Whisper's trailing "Hey," reaches into the next utterance's speech.
    let sheet = [
        spoken("U0381", 1368.36, 1369.24, &["Guess", "I'm", "fine."]),
        spoken("U0382", 1373.16, 1375.16, &["What's", "going", "on?"]),
    ];
    let lines = [
        line("U0381", "Guess I'm fine. Eh? Hey,", &[]),
        line("U0382", "Hey, what's going on?", &[]),
    ];
    let spans = [(1368.36, 1373.70), (1369.0, 1375.16)];
    let kept = kept(&sheet, &lines, &spans);
    assert_eq!((kept[0].start_s, kept[0].end_s), (1368.36, 1373.16));
    assert_eq!((kept[1].start_s, kept[1].end_s), (1373.16, 1375.16));
}

#[test]
fn an_extra_trailing_word_widens_the_end_only() {
    let sheet = [spoken("U1", 5.0, 6.0, &["It's", "closing"])];
    assert_eq!(
        window_of(&sheet, "It's closing in!", &[(4.0, 6.8)]),
        (5.0, 6.8)
    );
}

#[test]
fn blocks_close_at_pauses_after_twenty_seconds_and_at_dropped_lines() {
    let kept = vec![
        kept_at("U1", 0, 0.0, 10.0, &["a"]),
        kept_at("U2", 1, 10.1, 21.0, &["b"]),
        // 0.2 s gap: too short a pause.
        kept_at("U3", 2, 21.2, 25.0, &["c"]),
        // 0.5 s gap after 25 s: a block edge.
        kept_at("U4", 3, 25.5, 30.0, &["d"]),
        // U5 of the sheet was dropped: a forced edge.
        kept_at("U6", 5, 30.1, 31.0, &["e"]),
    ];
    assert_eq!(plan_blocks(&kept), vec![0..3, 3..4, 4..5]);
}

#[test]
fn a_block_closes_before_passing_sixty_seconds() {
    let kept = vec![
        kept_at("U1", 0, 0.0, 15.0, &["a"]),
        kept_at("U2", 1, 15.5, 58.0, &["b"]),
        kept_at("U3", 2, 58.4, 70.0, &["c"]),
    ];
    // 15 s at the first pause: under 20 s and 58 s with the next, so it stays open; the next
    // utterance would pass 60 s, so the block closes at the second pause.
    assert_eq!(plan_blocks(&kept), vec![0..2, 2..3]);
    assert!(plan_blocks(&[]).is_empty());
}

#[test]
fn audio_spans_are_padded_but_stop_halfway_to_the_neighbours() {
    let kept = vec![
        kept_at("U1", 0, 0.1, 2.0, &["a"]),
        kept_at("U2", 1, 2.2, 4.0, &["b"]),
        kept_at("U3", 2, 9.0, 9.9, &["c"]),
    ];
    assert_eq!(audio_span(&kept, 0..1, 100.0), TimeSpan::new(0.0, 2.1));
    let middle = audio_span(&kept, 1..2, 100.0);
    assert!((middle.start_s - 2.1).abs() < 1e-9 && (middle.end_s - 4.3).abs() < 1e-9);
    assert_eq!(audio_span(&kept, 2..3, 10.0), TimeSpan::new(8.7, 10.0));
}

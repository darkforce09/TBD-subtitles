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
    let kept = kept(&sheet, &lines);
    assert_eq!(
        kept.iter().map(|k| k.id.as_str()).collect::<Vec<_>>(),
        vec!["U1", "U5"]
    );
    assert!(kept[0].narrator);
    assert!(kept[1].unsure);
    assert_eq!(kept[1].sheet_index, 4);
    assert_eq!(kept[1].speaker_starts, vec![1]);
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

use job_model::outputs::{ChunkWords, TimeSpan};

use super::*;

fn transcript(engine: &str, words: &[(&str, f64)]) -> EngineTranscript {
    EngineTranscript {
        engine: engine.into(),
        input: "mix".into(),
        chunks: vec![ChunkWords {
            span: TimeSpan::new(0.0, 60.0),
            words: words
                .iter()
                .map(|(t, s)| TimedWord {
                    text: t.to_string(),
                    start_s: *s,
                    end_s: s + 0.3,
                    confidence: None,
                })
                .collect(),
        }],
    }
}

#[test]
fn disagreements_are_written_inline_and_agreements_locked() {
    let p = transcript(
        "p",
        &[
            ("Law,", 723.4),
            ("the", 723.8),
            ("Birdcage", 724.1),
            ("is", 724.5),
            ("closing!", 724.8),
        ],
    );
    let w = transcript(
        "w",
        &[
            ("Law,", 723.4),
            ("the", 723.8),
            ("bird", 724.1),
            ("is", 724.5),
            ("closing", 724.8),
            ("in!", 725.0),
        ],
    );
    let sheet = build(&p, &[&w], &["P", "W"]);
    assert_eq!(sheet.len(), 1);
    assert_eq!(
        sheet[0].line,
        "U0001 12:03.4 1.7s | Law, the {P:Birdcage|W:bird} is closing!{W:+in!}"
    );
    assert_eq!(sheet[0].locked, vec![true, true, false, true, true]);
    assert_eq!(
        sheet[0].hypotheses[1].1,
        vec!["Law,", "the", "bird", "is", "closing", "in!"]
    );
}

#[test]
fn a_word_the_other_engine_missed_shows_as_empty() {
    let p = transcript("p", &[("oh", 0.0), ("no", 0.4)]);
    let w = transcript("w", &[("no", 0.4)]);
    let sheet = build(&p, &[&w], &["P", "W"]);
    assert!(
        sheet[0].line.ends_with("| {P:oh|W:∅} no"),
        "{}",
        sheet[0].line
    );
}

#[test]
fn pauses_and_sentence_ends_cut_utterances() {
    let p = transcript(
        "p",
        &[("Run!", 0.0), ("Now", 0.6), ("we", 1.0), ("wait", 2.5)],
    );
    let sheet = build(&p, &[], &["P"]);
    let texts: Vec<Vec<String>> = sheet
        .iter()
        .map(|u| u.words.iter().map(|w| w.text.clone()).collect())
        .collect();
    assert_eq!(texts, vec![vec!["Run!"], vec!["Now", "we"], vec!["wait"]]);
    assert_eq!(sheet[2].id, "U0003");
}

fn close(a: (f64, f64), b: (f64, f64)) -> bool {
    (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
}

#[test]
fn words_another_engine_heard_before_the_backbone_start_the_first_span() {
    let p = transcript(
        "p",
        &[
            ("Shut", 256.68),
            ("your", 257.0),
            ("filthy", 257.4),
            ("mouths!", 258.22),
        ],
    );
    let w = transcript(
        "w",
        &[
            ("Yeah,", 254.12),
            ("that's", 254.5),
            ("right!", 254.9),
            ("Shut", 256.68),
            ("your", 257.0),
            ("filthy", 257.4),
            ("mouths!", 258.22),
        ],
    );
    let spans = heard_spans(&p, &[&w]);
    assert_eq!(spans.len(), 1);
    assert!(close(spans[0], (254.12, 258.52)), "{spans:?}");
    let sheet = build(&p, &[&w], &["P", "W"]);
    assert!(
        sheet[0].line.contains("| {W:+Yeah, that's right!} Shut"),
        "{}",
        sheet[0].line
    );
    assert!(close((sheet[0].start_s, sheet[0].end_s), (256.68, 258.52)));
}

#[test]
fn words_another_engine_heard_after_the_backbone_end_the_span() {
    let p = transcript("p", &[("It's", 1.0), ("closing", 1.3)]);
    let w = transcript("w", &[("It's", 1.0), ("closing", 1.3), ("in!", 1.7)]);
    assert!(close(heard_spans(&p, &[&w])[0], (1.0, 2.0)));
}

#[test]
fn with_the_backbone_alone_each_span_is_the_backbone_span() {
    let p = transcript(
        "p",
        &[("Run!", 0.0), ("Now", 0.6), ("we", 1.0), ("wait", 2.5)],
    );
    let spans = heard_spans(&p, &[]);
    let expected = [(0.0, 0.3), (0.6, 1.3), (2.5, 2.8)];
    assert_eq!(spans.len(), expected.len());
    for (span, want) in spans.iter().zip(expected) {
        assert!(close(*span, want), "{spans:?}");
    }
}

#[test]
fn there_is_one_span_per_utterance_of_the_sheet() {
    let p = transcript(
        "p",
        &[("Run!", 0.0), ("Now", 0.6), ("we", 1.0), ("wait", 2.5)],
    );
    let w = transcript(
        "w",
        &[("Go,", 0.0), ("run!", 0.2), ("Now", 0.6), ("wait", 2.5)],
    );
    let spans = heard_spans(&p, &[&w]);
    assert_eq!(spans.len(), build(&p, &[&w], &["P", "W"]).len());
    assert_eq!(spans.len(), 3);
    // Whisper's "Go," before the chunk and its later "run!" both widen the first utterance.
    assert!(close(spans[0], (0.0, 0.5)), "{spans:?}");
}

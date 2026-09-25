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

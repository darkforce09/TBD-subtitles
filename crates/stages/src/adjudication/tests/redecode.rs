use job_model::outputs::{ChunkWords, EngineTranscript, TimedWord};
use serde_json::{Value, json};

use inference::llm::{Completion, LlmError};

use super::*;

fn utterance(id: &str, start_s: f64, end_s: f64, words: &[&str]) -> Utterance {
    Utterance {
        id: id.into(),
        start_s,
        end_s,
        words: words
            .iter()
            .map(|t| TimedWord {
                text: t.to_string(),
                start_s,
                end_s,
                confidence: None,
            })
            .collect(),
        locked: vec![true; words.len()],
        line: format!("{id} | {}", words.join(" ")),
        hypotheses: vec![("P".into(), words.iter().map(|w| w.to_string()).collect())],
    }
}

fn line(id: &str, t: &str, f: &[&str]) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
    }
}

fn redecode(ids: &[&str], heard: &[&[&str]]) -> Redecode {
    Redecode {
        ids: ids.iter().map(|s| s.to_string()).collect(),
        transcript: EngineTranscript {
            engine: "e".into(),
            input: "vocals".into(),
            chunks: heard
                .iter()
                .map(|words| ChunkWords {
                    span: TimeSpan::new(0.0, 1.0),
                    words: words
                        .iter()
                        .map(|t| TimedWord {
                            text: t.to_string(),
                            start_s: 0.0,
                            end_s: 0.1,
                            confidence: None,
                        })
                        .collect(),
                })
                .collect(),
        },
    }
}

#[test]
fn only_unsure_lines_are_heard_again_with_padding_inside_the_video() {
    let sheet = [
        utterance("U1", 0.2, 2.0, &["a"]),
        utterance("U2", 5.0, 9.8, &["b"]),
    ];
    let lines = [line("U1", "A.", &["UNSURE"]), line("U2", "B.", &["NARR"])];
    let ids = unsure_ids(&lines);
    assert_eq!(ids, vec!["U1".to_string()]);
    let all = vec!["U1".to_string(), "U2".to_string()];
    let spans = spans(&sheet, &all, 10.0);
    assert_eq!(spans[0], TimeSpan::new(0.0, 2.5));
    assert_eq!(spans[1], TimeSpan::new(4.5, 10.0));
}

#[test]
fn alternatives_join_the_hypotheses_and_the_line() {
    let sheet = [
        utterance("U1", 0.0, 1.0, &["bird", "cage"]),
        utterance("U2", 2.0, 3.0, &["b"]),
    ];
    let p = redecode(&["U1"], &[&["Birdcage"]]);
    let w = redecode(&["U1"], &[&["bird", "cage!"]]);
    let alt = with_alternatives(&sheet, &[("p", &p), ("w", &w)]);
    assert!(
        alt[0]
            .line
            .ends_with("ALT p: \"Birdcage\" w: \"bird cage!\""),
        "{}",
        alt[0].line
    );
    assert_eq!(alt[0].hypotheses.len(), 3);
    assert_eq!(alt[1], sheet[1]);
}

#[test]
fn the_second_pass_message_holds_context_but_asks_only_the_unsure_ids() {
    let sheet = [
        utterance("U1", 0.0, 1.0, &["x"]),
        utterance("U2", 2.0, 3.0, &["y"]),
        utterance("U3", 4.0, 5.0, &["z"]),
    ];
    let first = [
        line("U1", "Hello.", &[]),
        line("U2", "Why?", &["UNSURE"]),
        line("U3", "Bye.", &[]),
    ];
    let message = user_message(&["Luffy"], &sheet, &first, &["U2".to_string()]);
    assert!(
        message.contains("CONTEXT U1: Hello.\nU2 | y\nCONTEXT U3: Bye.\n"),
        "{message}"
    );
    assert!(!message.contains("U1 | x"));
}

#[test]
fn merging_replaces_only_the_answered_ids() {
    let first = [
        line("U1", "a", &["UNSURE"]),
        line("U2", "b", &[]),
        line("U3", "c", &["UNSURE"]),
    ];
    let second = [line("U1", "A!", &[])];
    let merged = merge(&first, &second);
    assert_eq!(merged[0], line("U1", "A!", &[]));
    assert_eq!(merged[1], first[1]);
    assert_eq!(merged[2], first[2]);
}

/// Answers every listed id it sees on the second call only, to exercise the re-ask.
struct Forgetful {
    calls: usize,
}

impl LanguageModel for Forgetful {
    fn name(&self) -> String {
        "forgetful".into()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        _: &Value,
    ) -> Result<Completion, LlmError> {
        assert!(system.contains("Second pass."));
        self.calls += 1;
        let lines: Vec<Value> = if self.calls == 1 {
            vec![json!({"id": "U9", "t": "stray", "f": []})]
        } else {
            user.lines()
                .filter(|l| l.starts_with('U'))
                .map(|l| json!({"id": l.split(' ').next(), "t": "settled", "f": []}))
                .collect()
        };
        Ok(Completion {
            json: json!({"lines": lines}),
            input_tokens: 1,
            output_tokens: 1,
            cost_usd: None,
        })
    }
}

#[test]
fn ids_a_second_pass_leaves_out_are_asked_once_more_and_strays_are_dropped() {
    let sheet = [
        utterance("U1", 0.0, 1.0, &["x"]),
        utterance("U2", 2.0, 3.0, &["y"]),
    ];
    let first = [line("U1", "x", &["UNSURE"]), line("U2", "y", &[])];
    let mut model = Forgetful { calls: 0 };
    let result = readjudicate(&mut model, &sheet, &first, &["U1".to_string()], &["Luffy"]);
    assert_eq!(model.calls, 2);
    assert_eq!(result.lines, vec![line("U1", "settled", &[])]);
}

//! A scripted model and a small video modelled on Dressrosa 12, for Fix It's tests.

use std::sync::{Arc, Mutex};

use inference::llm::{Completion, LanguageModel, LlmError};
use job_model::outputs::{
    Aligned, AlignedUtterance, AlignedWord, ChunkWords, Corrections, EngineTranscript, Line,
    TimeSpan, TimedWord, TimingSource, Utterance,
};
use job_model::report::{QcCheck, QcFinding, QcReport};
use serde_json::Value;

use super::items::Answered;
use super::{Episode, Make};

/// One call as the model saw it.
#[derive(Debug, Clone)]
pub struct Call {
    pub system: String,
    pub user: String,
}

type Script = dyn Fn(&str, &str) -> Result<Value, String> + Send + Sync;

/// A model whose every answer comes from a script of the system prompt and the message.
pub struct Fake {
    calls: Arc<Mutex<Vec<Call>>>,
    script: Arc<Script>,
}

impl LanguageModel for Fake {
    fn name(&self) -> String {
        "fake".into()
    }

    fn complete_json(
        &mut self,
        system: &str,
        user: &str,
        _schema: &Value,
    ) -> Result<Completion, LlmError> {
        self.calls.lock().unwrap().push(Call {
            system: system.into(),
            user: user.into(),
        });
        (self.script)(system, user)
            .map(|json| Completion {
                json,
                input_tokens: 100,
                output_tokens: 10,
                cost_usd: Some(0.01),
            })
            .map_err(LlmError)
    }
}

/// A maker of scripted models, and the calls they all saw.
pub fn scripted(
    script: impl Fn(&str, &str) -> Result<Value, String> + Send + Sync + 'static,
) -> (Box<Make<'static>>, Arc<Mutex<Vec<Call>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let script: Arc<Script> = Arc::new(script);
    let seen = calls.clone();
    let make = move || {
        Box::new(Fake {
            calls: seen.clone(),
            script: script.clone(),
        }) as Box<dyn LanguageModel + Send>
    };
    (Box::new(make), calls)
}

/// The ids named as `### ID` in a message, in order.
pub fn asked_ids(user: &str) -> Vec<String> {
    user.lines()
        .filter_map(|l| l.strip_prefix("### "))
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_string)
        .collect()
}

/// A brief with no suspects.
pub fn brief_answer() -> Value {
    serde_json::json!({
        "show": "One Piece (anime), English dub",
        "episode": "Dressrosa arc, One Pace Dressrosa 12",
        "cast": ["Luffy", "Rebecca", "Sanji", "Violet"],
        "summary": "The Colosseum crowd jeers at Rebecca; Sanji meets Violet.",
        "speech_habits": [],
        "suspects": []
    })
}

/// A small video: the Colosseum crowd line only Whisper heard, the dropped "Uh" before a
/// question, and the short "Oh?", with lines around each.
pub struct Fixture {
    pub sheet: Vec<Utterance>,
    pub lines: Vec<Line>,
    pub corrections: Corrections,
    pub qc: QcReport,
    pub timing: Aligned,
    pub heard: EngineTranscript,
    pub answered: Answered,
}

fn utterance(id: &str, start_s: f64, end_s: f64, p: &str, w: &str) -> Utterance {
    let words: Vec<TimedWord> = p
        .split_whitespace()
        .map(|t| TimedWord {
            text: t.into(),
            start_s,
            end_s,
            confidence: None,
        })
        .collect();
    let split = |s: &str| s.split_whitespace().map(str::to_string).collect::<Vec<_>>();
    Utterance {
        id: id.into(),
        start_s,
        end_s,
        locked: vec![true; words.len()],
        words,
        line: format!("{id} | {p}"),
        hypotheses: vec![("P".into(), split(p)), ("W".into(), split(w))],
    }
}

fn line(id: &str, t: &str, f: &[&str]) -> Line {
    Line {
        id: id.into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
    }
}

fn timed(id: &str, text: &str, placed: TimingSource) -> AlignedUtterance {
    AlignedUtterance {
        id: id.into(),
        words: text
            .split_whitespace()
            .map(|t| AlignedWord {
                text: t.into(),
                start_s: 0.0,
                end_s: 0.0,
                source: placed,
            })
            .collect(),
        speaker_starts: Vec::new(),
        new_speaker: false,
        narrator: false,
        unsure: false,
    }
}

/// A finding about line `id`.
pub fn finding(check: QcCheck, id: &str, time_s: f64, text: &str, detail: &str) -> QcFinding {
    QcFinding {
        check,
        time_s,
        text: text.into(),
        detail: detail.into(),
        utterance: Some(id.into()),
    }
}

impl Fixture {
    pub fn new() -> Fixture {
        let crowd = "Yeah, that's right! We want to see her suffer!";
        let sheet = vec![
            utterance("U0060", 250.0, 252.0, "Look at her.", "Look at her."),
            utterance(
                "U0061",
                256.7,
                258.5,
                "Shut your filthy mouths!",
                &format!("{crowd} Shut your filthy mouths!"),
            ),
            utterance("U0062", 259.0, 261.0, "Rebecca!", "Rebecca!"),
            utterance(
                "U0294",
                1100.6,
                1103.1,
                "There isn't a person alive who can catch me.",
                "There isn't a person alive who can catch me.",
            ),
            utterance(
                "U0295",
                1103.4,
                1106.6,
                "Uh does that mean you're gonna stay in this country?",
                "Does that mean you're gonna stay in this country?",
            ),
            utterance("U0296", 1107.0, 1108.0, "Maybe.", "Maybe."),
            utterance(
                "U0313",
                1153.8,
                1156.0,
                "I think I'll pass on the panties this time.",
                "I think I'll pass on the panties this time.",
            ),
            utterance("U0314", 1156.3, 1156.5, "Oh?", "Oh?"),
            utterance("U0315", 1156.9, 1157.5, "Panties?", "Panties?"),
        ];
        let lines = vec![
            line("U0060", "Look at her.", &[]),
            line(
                "U0061",
                &format!("{crowd} Shut your filthy mouths!"),
                &["SPK"],
            ),
            line("U0062", "Rebecca!", &["SPK"]),
            line("U0294", "There isn't a person alive who can catch me.", &[]),
            line(
                "U0295",
                "Does that mean you're gonna stay in this country?",
                &["SPK"],
            ),
            line("U0296", "Maybe.", &["SPK"]),
            line("U0313", "I think I'll pass on the panties this time.", &[]),
            line("U0314", "Oh?", &["SPK"]),
            line("U0315", "Panties?", &[]),
        ];
        let timing = Aligned {
            utterances: vec![
                AlignedUtterance {
                    words: [
                        timed("U0061", crowd, TimingSource::Interpolated).words,
                        timed("U0061", "Shut your filthy mouths!", TimingSource::Ctc).words,
                    ]
                    .concat(),
                    ..timed("U0061", "", TimingSource::Ctc)
                },
                timed(
                    "U0295",
                    "Does that mean you're gonna stay in this country?",
                    TimingSource::Ctc,
                ),
                timed("U0314", "Oh?", TimingSource::Ctc),
            ],
            ..Aligned::default()
        };
        let uncovered = QcFinding {
            check: QcCheck::UncoveredSpeech,
            time_s: 1103.2,
            text: String::new(),
            detail: "1.2 s".into(),
            utterance: None,
        };
        let qc = QcReport {
            findings: vec![
                finding(QcCheck::TooShort, "U0061", 256.2, crowd, "13 frames"),
                uncovered,
                finding(QcCheck::TooShort, "U0314", 1156.1, "Oh?", "17 frames"),
            ],
            ..QcReport::default()
        };
        let heard = EngineTranscript {
            engine: "parakeet".into(),
            input: "vocals".into(),
            chunks: vec![ChunkWords {
                span: TimeSpan::new(1100.0, 1110.0),
                words: vec![TimedWord {
                    text: "Uh".into(),
                    start_s: 1103.5,
                    end_s: 1103.8,
                    confidence: None,
                }],
            }],
        };
        Fixture {
            sheet,
            lines,
            corrections: Corrections::default(),
            qc,
            timing,
            heard,
            answered: Answered::none(),
        }
    }

    pub fn episode<'a>(&'a self, glossary: &'a [&'a str]) -> Episode<'a> {
        Episode {
            video_name: "[Muhn Pace] Dressrosa 12",
            folder_name: "one_pace",
            glossary_name: "one_piece",
            glossary,
            sheet: &self.sheet,
            lines: &self.lines,
            corrections: &self.corrections,
            qc: &self.qc,
            timing: &self.timing,
            heard: &self.heard,
            answered: &self.answered,
        }
    }
}

/// The glossary the fixture uses.
pub const GLOSSARY: [&str; 3] = ["Rebecca", "Violet", "Colosseum"];

/// Each line a repair message asks about, with its text and flags as it stands now.
pub fn asked_lines(user: &str) -> Vec<(String, String, Vec<String>)> {
    let mut out = Vec::new();
    let mut id = None;
    for l in user.lines() {
        if let Some(rest) = l.strip_prefix("### ") {
            id = rest.split_whitespace().next().map(str::to_string);
        } else if let Some(rest) = l.strip_prefix("Now: ")
            && let Some(current) = id.take()
        {
            let (flags, text) = match rest.strip_prefix('[') {
                Some(flagged) => {
                    let (flags, text) = flagged.split_once("] ").unwrap_or((flagged, ""));
                    (flags.split_whitespace().map(str::to_string).collect(), text)
                }
                None => (Vec::new(), rest),
            };
            out.push((current, text.trim_matches('"').to_string(), flags));
        }
    }
    out
}

/// A repair answer: each asked line as `change` returns it, or as it stands when it returns
/// `None`, with a reason.
pub fn repair_answer(
    user: &str,
    change: impl Fn(&str) -> Option<(&'static str, Vec<&'static str>)>,
) -> Value {
    let lines: Vec<Value> = asked_lines(user)
        .into_iter()
        .map(|(id, text, flags)| {
            let (t, f): (String, Vec<String>) = match change(&id) {
                Some((t, f)) => (t.into(), f.into_iter().map(str::to_string).collect()),
                None => (text, flags),
            };
            serde_json::json!({"id": id, "t": t, "f": f, "why": format!("reason for {id}")})
        })
        .collect();
    serde_json::json!({ "lines": lines })
}

/// A judge answer: `accept` gives each asked line's verdict, `None` leaves it out.
pub fn judge_answer(user: &str, accept: impl Fn(&str) -> Option<bool>) -> Value {
    let verdicts: Vec<Value> = asked_ids(user)
        .into_iter()
        .filter_map(|id| {
            accept(&id).map(|ok| serde_json::json!({"id": id, "accept": ok, "why": "checked"}))
        })
        .collect();
    serde_json::json!({ "verdicts": verdicts })
}

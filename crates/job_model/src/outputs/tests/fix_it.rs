use crate::job::{JobSettings, StepMeasure, StepRecord};
use crate::report::{QcFinding, QcSummary};

use super::*;

fn line(verdict: FixVerdict) -> LineFix {
    LineFix {
        id: "U0295".into(),
        problems: vec!["heard speech over 1 s with no cue".into()],
        checks: vec![QcCheck::UncoveredSpeech],
        before_text: "Does that mean you're gonna stay?".into(),
        before_flags: vec!["SPK".into()],
        after_text: "Uh, does that mean you're gonna stay?".into(),
        after_flags: vec!["SPK".into()],
        steps: vec![FixStep {
            family: FixFamily::Timing,
            text: "Uh, does that mean you're gonna stay?".into(),
            flags: vec!["SPK".into()],
            why: "P heard Uh where the subtitle starts late.".into(),
        }],
        refused: Vec::new(),
        removed: Vec::new(),
        verdict,
        applied: false,
    }
}

#[test]
fn only_kept_and_accepted_lines_get_a_correction() {
    let why = || "reason".to_string();
    assert!(FixVerdict::Kept { why: why() }.writes_correction());
    assert!(FixVerdict::Accepted { why: why() }.writes_correction());
    assert!(!FixVerdict::TurnedDown { why: why() }.writes_correction());
    assert!(!FixVerdict::NotJudged { why: why() }.writes_correction());
    assert!(!FixVerdict::Unchanged.writes_correction());
    assert!(!FixVerdict::NotAnswered { why: why() }.writes_correction());
}

#[test]
fn a_line_is_answered_unless_its_calls_or_its_judge_failed() {
    let why = || "reason".to_string();
    assert!(FixVerdict::Unchanged.answered());
    assert!(FixVerdict::Kept { why: why() }.answered());
    assert!(FixVerdict::Accepted { why: why() }.answered());
    assert!(FixVerdict::TurnedDown { why: why() }.answered());
    assert!(!FixVerdict::NotJudged { why: why() }.answered());
    assert!(!FixVerdict::NotAnswered { why: why() }.answered());
}

#[test]
fn the_owner_reads_the_steps_reasons_or_the_kept_reason() {
    let accepted = line(FixVerdict::Accepted {
        why: "fits the scene".into(),
    });
    assert!(accepted.changed());
    assert_eq!(accepted.why(), "P heard Uh where the subtitle starts late.");
    let kept = line(FixVerdict::Kept {
        why: "Re-timed alone.".into(),
    });
    assert_eq!(kept.why(), "Re-timed alone.");
}

#[test]
fn a_record_round_trips_through_json() {
    let record = FixRecord {
        model: "opus".into(),
        video: "[Muhn Pace] Dressrosa 12".into(),
        brief: FixBrief {
            show: "One Piece".into(),
            suspects: vec![Suspect {
                id: "U0061".into(),
                why: "crowd line".into(),
            }],
            ..FixBrief::default()
        },
        lines: vec![line(FixVerdict::Accepted { why: "ok".into() })],
        calls: 4,
        adjudication: "abc".into(),
        before: Some(FixBefore {
            counts: BTreeMap::from([(QcCheck::TooFast, 3)]),
            ..FixBefore::default()
        }),
        ..FixRecord::default()
    };
    let json = serde_json::to_string(&record).unwrap();
    assert!(json.contains(r#""family":"timing""#), "{json}");
    assert!(json.contains(r#""accepted":{"why":"ok"}"#), "{json}");
    let back: FixRecord = serde_json::from_str(&json).unwrap();
    assert_eq!(back, record);
}

#[test]
fn a_record_written_before_checks_adjudication_and_before_loads() {
    let json = r#"{
        "model": "opus", "video": "v",
        "brief": {"show": "", "episode": "", "cast": [], "summary": "",
                  "speech_habits": [], "suspects": []},
        "lines": [{"id": "U1", "problems": ["too short"], "before_text": "Go!",
                   "before_flags": [], "after_text": "Go!", "after_flags": [],
                   "verdict": "unchanged"}],
        "calls": 2, "input_tokens": 10, "output_tokens": 1, "cost_usd": 0.5
    }"#;
    let record: FixRecord = serde_json::from_str(json).unwrap();
    assert!(record.lines[0].checks.is_empty());
    assert_eq!(record.adjudication, "");
    assert_eq!(record.before, None);
}

#[test]
fn before_keeps_the_counts_the_first_uncovered_speech_and_the_reading_speed() {
    let finding = |check, time_s| QcFinding {
        check,
        time_s,
        text: String::new(),
        detail: String::new(),
        utterance: None,
    };
    let qc = QcReport {
        summary: QcSummary {
            cues: 120,
            cps_ok_share: 0.9,
            ..QcSummary::default()
        },
        findings: vec![
            finding(QcCheck::TooShort, 3.0),
            finding(QcCheck::UncoveredSpeech, 12.5),
            finding(QcCheck::TooShort, 20.0),
            finding(QcCheck::UncoveredSpeech, 40.0),
        ],
    };
    let before = FixBefore::of(&qc);
    assert_eq!(
        before.counts,
        BTreeMap::from([(QcCheck::TooShort, 2), (QcCheck::UncoveredSpeech, 2)])
    );
    assert_eq!(before.first_uncovered_s, Some(12.5));
    assert_eq!((before.cps_ok_share, before.cues), (0.9, 120));
    assert_eq!(FixBefore::of(&QcReport::default()), FixBefore::default());
    let json = serde_json::to_string(&before).unwrap();
    assert!(json.contains(r#""too_short":2"#), "{json}");
    assert_eq!(serde_json::from_str::<FixBefore>(&json).unwrap(), before);
}

#[test]
fn a_record_is_current_until_the_video_is_adjudicated_again() {
    let job = |fingerprint: Option<&str>| JobRecord {
        video: "v.mp4".into(),
        video_size: 0,
        video_modified_s: 0,
        settings: JobSettings::with_glossary(Vec::new()),
        models_dir: None,
        corrections: None,
        steps: fingerprint
            .map(|f| {
                let step = StepRecord {
                    fingerprint: f.into(),
                    finished_ns: 0,
                    measure: StepMeasure::default(),
                };
                BTreeMap::from([(StepName::Readjudicate, step)])
            })
            .unwrap_or_default(),
    };
    let record = |adjudication: &str| FixRecord {
        adjudication: adjudication.into(),
        ..FixRecord::default()
    };
    assert!(record("").is_current(&job(Some("abc"))));
    assert!(record("").is_current(&job(None)));
    assert!(record("abc").is_current(&job(Some("abc"))));
    assert!(!record("abc").is_current(&job(Some("def"))));
    assert!(!record("abc").is_current(&job(None)));
}

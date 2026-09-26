use super::*;

fn finding(check: QcCheck) -> QcFinding {
    QcFinding {
        check,
        time_s: 1.0,
        text: "text".into(),
        detail: String::new(),
        utterance: None,
    }
}

fn report(findings: Vec<QcFinding>, cps_ok_share: f64) -> QcReport {
    QcReport {
        summary: QcSummary {
            cues: 10,
            cps_ok_share,
            ..QcSummary::default()
        },
        findings,
    }
}

#[test]
fn findings_for_review_never_fail_a_job() {
    let r = report(
        vec![
            finding(QcCheck::TooFast),
            finding(QcCheck::Novel),
            finding(QcCheck::Unsure),
            finding(QcCheck::WeakTiming),
            finding(QcCheck::GapTooSmall),
        ],
        0.99,
    );
    assert!(r.passes(), "{:?}", r.failures());
}

#[test]
fn layout_violations_uncovered_speech_failed_calls_and_offset_fail_a_job() {
    let r = report(
        vec![
            finding(QcCheck::Overlap),
            finding(QcCheck::TooShort),
            finding(QcCheck::UncoveredSpeech),
            finding(QcCheck::FailedCall),
            finding(QcCheck::Offset),
        ],
        0.99,
    );
    let failures = r.failures();
    assert_eq!(failures.len(), 4, "{failures:?}");
    assert!(failures[0].starts_with("2 layout"));
    assert!(!r.passes());
}

#[test]
fn too_few_cues_within_the_reading_speed_fail_a_job() {
    assert!(!report(vec![], 0.94).passes());
    assert!(report(vec![], 0.95).passes());
}

#[test]
fn a_finding_without_an_utterance_parses() {
    let json = r#"{"check":"too_fast","time_s":2.0,"text":"t","detail":"23.4 cps"}"#;
    let finding: QcFinding = serde_json::from_str(json).expect("parse");
    assert_eq!(finding.utterance, None);
}

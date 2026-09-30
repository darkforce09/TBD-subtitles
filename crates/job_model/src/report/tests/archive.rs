use std::collections::BTreeMap;

use super::{QcCheck, QcFinding, QcReport, QcSummary};
use crate::archive_round_trip::round_trip;

const EVERY_CHECK: [QcCheck; 16] = [
    QcCheck::Overlap,
    QcCheck::GapTooSmall,
    QcCheck::TooShort,
    QcCheck::TooLong,
    QcCheck::TooFast,
    QcCheck::LineTooLong,
    QcCheck::TooManyLines,
    QcCheck::Empty,
    QcCheck::PastEnd,
    QcCheck::UncoveredSpeech,
    QcCheck::Unsure,
    QcCheck::Novel,
    QcCheck::RemovedLocked,
    QcCheck::WeakTiming,
    QcCheck::Offset,
    QcCheck::FailedCall,
];

/// Fails to compile when `QcCheck` gains a variant, until [`EVERY_CHECK`] lists it too.
fn listed(check: QcCheck) {
    match check {
        QcCheck::Overlap
        | QcCheck::GapTooSmall
        | QcCheck::TooShort
        | QcCheck::TooLong
        | QcCheck::TooFast
        | QcCheck::LineTooLong
        | QcCheck::TooManyLines
        | QcCheck::Empty
        | QcCheck::PastEnd
        | QcCheck::UncoveredSpeech
        | QcCheck::Unsure
        | QcCheck::Novel
        | QcCheck::RemovedLocked
        | QcCheck::WeakTiming
        | QcCheck::Offset
        | QcCheck::FailedCall => {}
    }
}

fn finding(check: QcCheck) -> QcFinding {
    QcFinding {
        check,
        time_s: 61.25,
        text: "I'm gonna be King of the Pirates!".into(),
        detail: "23.4 cps".into(),
        utterance: Some("U0412".into()),
    }
}

fn summary() -> QcSummary {
    QcSummary {
        cues: 420,
        dialogue_cues: 380,
        sound_cues: 30,
        music_cues: 10,
        cps_ok_share: 0.97,
        words_by_source: BTreeMap::from([("ctc".into(), 3000), ("backbone".into(), 12)]),
        unsure: 2,
        novel: 1,
        offset_ms: Some(-4.5),
        uncovered_speech_s: 0.5,
        voice_without_cue_s: 3.25,
        video_s: 1440.0,
        reviewed: 5,
        fixed: 3,
    }
}

#[test]
fn every_qc_check_round_trips() {
    for check in EVERY_CHECK {
        listed(check);
        round_trip(&check);
    }
}

#[test]
fn qc_finding_round_trips() {
    round_trip(&finding(QcCheck::TooFast));
}

#[test]
fn qc_summary_round_trips() {
    round_trip(&summary());
}

#[test]
fn qc_report_round_trips() {
    round_trip(&QcReport {
        summary: summary(),
        findings: EVERY_CHECK.into_iter().map(finding).collect(),
    });
}

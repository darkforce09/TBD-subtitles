use job_model::outputs::{Chosen, Correction};
use job_model::report::{QcFinding, QcSummary};

use super::*;

fn finding(check: QcCheck, time_s: f64, line: Option<&str>) -> QcFinding {
    QcFinding {
        check,
        time_s,
        text: String::new(),
        detail: String::new(),
        utterance: line.map(str::to_string),
    }
}

fn corrected(ids: &[&str]) -> Corrections {
    Corrections {
        lines: ids
            .iter()
            .map(|id| Correction {
                id: (*id).to_string(),
                text: "text".into(),
                flags: Vec::new(),
                chosen: Chosen::Typed,
            })
            .collect(),
    }
}

/// A report that passes: ten cues, all easy to read, and findings for review only.
fn passing() -> QcReport {
    QcReport {
        summary: QcSummary {
            cues: 10,
            cps_ok_share: 1.0,
            ..QcSummary::default()
        },
        findings: vec![
            finding(QcCheck::Unsure, 1.0, Some("U1")),
            finding(QcCheck::WeakTiming, 2.0, Some("U2")),
        ],
    }
}

#[test]
fn lines_are_counted_once_per_group_and_checked_when_corrected() {
    let qc = QcReport {
        findings: vec![
            finding(QcCheck::Unsure, 1.0, Some("U1")),
            finding(QcCheck::TooFast, 1.0, Some("U1")),
            finding(QcCheck::TooFast, 3.0, Some("U2")),
            finding(QcCheck::TooFast, 4.0, Some("U2")),
            finding(QcCheck::RemovedLocked, 5.0, Some("U3")),
            finding(QcCheck::TooShort, 6.0, None),
            finding(QcCheck::UncoveredSpeech, 7.0, None),
            finding(QcCheck::FailedCall, 0.0, None),
        ],
        ..QcReport::default()
    };
    let counts = line_counts(&qc, &corrected(&["U2", "U9"]));
    assert_eq!(
        counts.flagged, 4,
        "U1, U2, U3 and the corrected U9; a sound cue is no line"
    );
    assert_eq!(counts.checked, 2, "every corrected line is checked");
    assert_eq!(counts.to_check(), 2);
    assert_eq!(
        counts.groups,
        [
            (LineGroup::Unsure, 1),
            (LineGroup::HeardWordReplaced, 1),
            (LineGroup::TooFast, 2),
        ]
    );
    let row = summary(&qc, &corrected(&["U2"]));
    assert_eq!(
        row,
        RowSummary {
            problems: 3,
            flagged: 3,
            to_check: 2,
        },
        "layout, speech with no subtitle and the failed call"
    );
    assert!(!row.passes());
    assert_eq!(
        line_counts(&QcReport::default(), &Corrections::default()),
        LineCounts::default()
    );
}

#[test]
fn problems_are_empty_exactly_when_the_job_passes_for_each_rule() {
    let good = passing();
    assert!(good.passes());
    assert!(problems(&good).is_empty(), "review findings fail nothing");
    let mut rules: Vec<(QcReport, Problem)> = Vec::new();
    let with = |extra: Vec<QcFinding>| {
        let mut qc = passing();
        qc.findings.extend(extra);
        qc
    };
    rules.push((
        with(vec![
            finding(QcCheck::Overlap, 3.0, Some("U3")),
            finding(QcCheck::PastEnd, 9.0, None),
        ]),
        Problem::Layout(2),
    ));
    rules.push((
        with(vec![
            finding(QcCheck::UncoveredSpeech, 8.0, None),
            finding(QcCheck::UncoveredSpeech, 4.0, None),
        ]),
        Problem::UncoveredSpeech(4.0),
    ));
    rules.push((
        with(vec![finding(QcCheck::Offset, 0.0, None)]),
        Problem::Offset,
    ));
    rules.push((
        with(vec![finding(QcCheck::FailedCall, 0.0, None)]),
        Problem::FailedCall(1),
    ));
    let mut slow = passing();
    slow.summary.cps_ok_share = 0.9;
    rules.push((slow, Problem::ReadingSpeed(0.9)));
    for (qc, expected) in &rules {
        assert!(!qc.passes(), "{expected:?} fails the job");
        assert_eq!(problems(qc), [*expected]);
        assert_eq!(problems(qc).len(), qc.failures().len());
    }
    let mut every = passing();
    for (qc, _) in &rules {
        every.findings.extend(qc.findings.iter().skip(2).cloned());
    }
    every.summary.cps_ok_share = 0.9;
    assert_eq!(problems(&every).len(), 5);
    assert_eq!(problems(&every).len(), every.failures().len());
    let mut empty = QcReport::default();
    empty.summary.cps_ok_share = 0.0;
    assert!(
        empty.passes() && problems(&empty).is_empty(),
        "no cues, no reading-speed problem"
    );
}

#[test]
fn a_group_opens_at_its_earliest_line() {
    let qc = QcReport {
        findings: vec![
            finding(QcCheck::Unsure, 1.0, Some("U1")),
            finding(QcCheck::TooShort, 2.0, None),
            finding(QcCheck::Overlap, 3.0, Some("U4")),
            finding(QcCheck::Overlap, 5.0, Some("U7")),
        ],
        ..QcReport::default()
    };
    assert_eq!(first_line(&qc, LineGroup::Layout).as_deref(), Some("U4"));
    assert_eq!(first_line(&qc, LineGroup::TooFast), None);
}

#[test]
fn corrected_lines_stay_counted_after_the_correction_run_settles_them() {
    let before = QcReport {
        findings: vec![
            finding(QcCheck::Unsure, 1.0, Some("U1")),
            finding(QcCheck::Novel, 2.0, Some("U2")),
            finding(QcCheck::TooFast, 3.0, Some("U3")),
        ],
        ..QcReport::default()
    };
    let corrections = corrected(&["U1"]);
    let saved = line_counts(&before, &corrections);
    assert_eq!((saved.checked, saved.flagged), (1, 3), "1 of 3 checked");
    // The correction run settles U1's unsure finding: the check no longer names it.
    let after = QcReport {
        findings: before.findings[1..].to_vec(),
        ..QcReport::default()
    };
    let run = line_counts(&after, &corrections);
    assert_eq!((run.checked, run.flagged), (1, 3), "still 1 of 3 checked");
    assert_eq!(
        run.groups,
        [(LineGroup::NovelWord, 1), (LineGroup::TooFast, 1)],
        "the settled line is under no group"
    );
    let all = corrected(&["U1", "U2", "U3"]);
    let done = line_counts(&QcReport::default(), &all);
    assert_eq!(
        (done.checked, done.flagged, done.to_check()),
        (3, 3, 0),
        "a video whose every line was corrected stays all checked"
    );
}

use job_model::outputs::{Chosen, Correction, FixBefore, FixRecord, FixVerdict, LineFix};
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
    let counts = line_counts(&qc, &corrected(&["U2", "U9"]), &Answered::none());
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
    let row = summary(&qc, &corrected(&["U2"]), &Answered::none(), false);
    assert_eq!(
        row,
        RowSummary {
            problems: 3,
            flagged: 3,
            to_check: 2,
            fixed_by_claude: false,
        },
        "layout, speech with no subtitle and the failed call"
    );
    assert!(!row.passes());
    assert_eq!(
        line_counts(
            &QcReport::default(),
            &Corrections::default(),
            &Answered::none()
        ),
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
    let none = Answered::none();
    let saved = line_counts(&before, &corrections, &none);
    assert_eq!((saved.checked, saved.flagged), (1, 3), "1 of 3 checked");
    // The correction run settles U1's unsure finding: the check no longer names it.
    let after = QcReport {
        findings: before.findings[1..].to_vec(),
        ..QcReport::default()
    };
    let run = line_counts(&after, &corrections, &none);
    assert_eq!((run.checked, run.flagged), (1, 3), "still 1 of 3 checked");
    assert_eq!(
        run.groups,
        [(LineGroup::NovelWord, 1), (LineGroup::TooFast, 1)],
        "the settled line is under no group"
    );
    let all = corrected(&["U1", "U2", "U3"]);
    let done = line_counts(&QcReport::default(), &all, &none);
    assert_eq!(
        (done.checked, done.flagged, done.to_check()),
        (3, 3, 0),
        "a video whose every line was corrected stays all checked"
    );
}

/// A Fix It record's answers: each line with the checks it was asked about, with `verdict`.
fn answered(lines: &[(&str, &[QcCheck])], verdict: FixVerdict) -> Answered {
    let record = FixRecord {
        lines: lines
            .iter()
            .map(|(id, checks)| LineFix {
                id: (*id).to_string(),
                problems: Vec::new(),
                checks: checks.to_vec(),
                before_text: "Go!".into(),
                before_flags: Vec::new(),
                after_text: "Go!".into(),
                after_flags: Vec::new(),
                steps: Vec::new(),
                refused: Vec::new(),
                removed: Vec::new(),
                verdict: verdict.clone(),
                applied: false,
            })
            .collect(),
        ..FixRecord::default()
    };
    Answered::from_record(&record)
}

#[test]
fn claude_checks_its_changes_and_the_lines_whose_every_finding_it_answered() {
    let qc = QcReport {
        findings: vec![
            finding(QcCheck::Unsure, 1.0, Some("U1")),
            finding(QcCheck::Unsure, 2.0, Some("U2")),
            finding(QcCheck::TooFast, 2.0, Some("U2")),
            finding(QcCheck::Novel, 3.0, Some("U3")),
            finding(QcCheck::Unsure, 4.0, Some("U4")),
            finding(QcCheck::TooShort, 5.0, Some("U5")),
        ],
        ..QcReport::default()
    };
    let mut corrections = corrected(&["U4"]);
    corrections.lines.push(Correction {
        id: "U5".into(),
        text: "Go on!".into(),
        flags: Vec::new(),
        chosen: Chosen::FixIt {
            model: "opus".into(),
            why: "Timed again.".into(),
        },
    });
    let asked: &[(&str, &[QcCheck])] = &[
        ("U1", &[QcCheck::Unsure]),
        ("U2", &[QcCheck::Unsure]),
        ("U4", &[QcCheck::Unsure]),
    ];
    let answered_now = answered(asked, FixVerdict::Unchanged);
    let counts = line_counts(&qc, &corrections, &answered_now);
    assert_eq!(counts.flagged, 5);
    assert_eq!(counts.checked, 1, "the owner corrected U4");
    assert_eq!(
        counts.by_claude, 2,
        "U1, answered, and U5, changed; U2's too-fast finding is open, U4 is the owner's"
    );
    assert_eq!(counts.to_check(), 2, "U2 and U3");
    assert_eq!(
        counts.groups.first(),
        Some(&(LineGroup::ChangedByFixIt, 1)),
        "U5 waits for the owner in Claude's group"
    );
    let asked_before = fixable(&qc, &corrections, &Answered::none());
    let asked_after = fixable(&qc, &corrections, &answered_now);
    assert_eq!(
        asked_before - asked_after,
        2,
        "the answered findings of U1 and U2 are not asked again"
    );
    assert!(asked_after > 0, "U2's too-fast finding and U3 are asked");
    let failed = answered(
        asked,
        FixVerdict::NotAnswered {
            why: "the call failed".into(),
        },
    );
    assert_eq!(
        line_counts(&qc, &corrections, &failed).by_claude,
        1,
        "a line whose calls failed is not checked"
    );
    let row = summary(&qc, &corrections, &answered_now, true);
    assert_eq!((row.to_check, row.fixed_by_claude), (2, true));
}

#[test]
fn the_problems_of_a_record_before_fix_it_are_the_check_s() {
    let mut qc = passing();
    qc.findings.push(finding(QcCheck::Overlap, 3.0, Some("U3")));
    qc.findings
        .push(finding(QcCheck::UncoveredSpeech, 8.0, None));
    qc.summary.cps_ok_share = 0.9;
    let before = FixBefore::of(&qc);
    assert_eq!(
        problems_of(
            &before.counts,
            before.first_uncovered_s,
            before.cps_ok_share,
            before.cues
        ),
        problems(&qc)
    );
}

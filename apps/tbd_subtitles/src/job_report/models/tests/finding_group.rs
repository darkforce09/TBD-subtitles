use super::*;

/// Every check the quality check makes.
const CHECKS: [QcCheck; 16] = [
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

#[test]
fn every_check_about_a_line_has_a_group_and_the_job_wide_ones_none() {
    let job_wide = [
        QcCheck::UncoveredSpeech,
        QcCheck::Offset,
        QcCheck::FailedCall,
    ];
    for check in CHECKS {
        assert_eq!(
            LineGroup::of(check).is_none(),
            job_wide.contains(&check),
            "{check:?}"
        );
        if check.is_layout_violation() {
            assert_eq!(LineGroup::of(check), Some(LineGroup::Layout), "{check:?}");
        }
    }
    assert_eq!(
        LineGroup::of(QcCheck::RemovedLocked),
        Some(LineGroup::HeardWordReplaced),
        "the agreed word dropped is a heard word replaced"
    );
}

#[test]
fn each_group_has_its_own_title_chip_and_explanation() {
    let titles: Vec<&str> = LineGroup::ALL.iter().map(|g| g.title()).collect();
    assert_eq!(
        titles,
        [
            "Changed by Claude",
            "Unsure what was said",
            "Heard word replaced",
            "Word no engine heard",
            "Too fast to read",
            "Loosely timed",
            "Layout",
        ]
    );
    let chips: Vec<&str> = LineGroup::ALL.iter().map(|g| g.chip()).collect();
    assert_eq!(
        chips,
        [
            "Claude",
            "Unsure",
            "Word replaced",
            "Word no engine heard",
            "Too fast",
            "Loosely timed",
            "Layout",
        ]
    );
    for group in LineGroup::ALL {
        assert!(group.explanation().ends_with('.'), "{group:?}");
        if group == LineGroup::ChangedByFixIt {
            // Its lines come from the corrections, never from a check.
            assert!(
                CHECKS
                    .iter()
                    .all(|check| LineGroup::of(*check) != Some(group))
            );
            continue;
        }
        assert!(
            CHECKS
                .iter()
                .any(|check| LineGroup::of(*check) == Some(group)),
            "{group:?} holds some check"
        );
    }
}

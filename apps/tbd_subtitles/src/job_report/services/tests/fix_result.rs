use std::collections::BTreeMap;

use job_model::outputs::{Correction, FixBefore};
use job_model::report::QcCheck;

use super::*;

/// A line Fix It asked about, `before` then `after`, with `verdict`, `applied` or not.
fn line(id: &str, before: &str, after: &str, verdict: FixVerdict, applied: bool) -> LineFix {
    LineFix {
        id: id.into(),
        problems: Vec::new(),
        checks: vec![QcCheck::Unsure],
        before_text: before.into(),
        before_flags: Vec::new(),
        after_text: after.into(),
        after_flags: Vec::new(),
        steps: Vec::new(),
        refused: Vec::new(),
        removed: Vec::new(),
        verdict,
        applied,
    }
}

fn accepted() -> FixVerdict {
    FixVerdict::Accepted {
        why: "Both engines heard it.".into(),
    }
}

fn correction(id: &str, chosen: Chosen) -> Correction {
    Correction {
        id: id.into(),
        text: "text".into(),
        flags: Vec::new(),
        chosen,
    }
}

/// A run over seven lines: four changes that went in (U4 kept by the owner, U3 undone), one the
/// owner's own correction kept out, one turned down, one already right, one never answered.
fn record() -> FixRecord {
    FixRecord {
        model: "opus".into(),
        lines: vec![
            line("U7", "Go!", "Go on!", accepted(), true),
            line("U2", "Yeah, go!", "Go!", accepted(), true),
            line("U1", "Go!", "Uh, go!", accepted(), true),
            line("U4", "Franky!", "Frankie!", accepted(), true),
            line("U3", "Wait!", "Wait up!", accepted(), true),
            line("U5", "Stop!", "Halt!", accepted(), false),
            line(
                "U6",
                "Run!",
                "Run!",
                FixVerdict::TurnedDown {
                    why: "It paraphrases.".into(),
                },
                false,
            ),
            line("U8", "Hi!", "Hi!", FixVerdict::Unchanged, false),
            line(
                "U9",
                "Bye!",
                "Bye!",
                FixVerdict::NotAnswered {
                    why: "the call failed".into(),
                },
                false,
            ),
        ],
        before: Some(FixBefore {
            counts: BTreeMap::from([(QcCheck::TooShort, 2), (QcCheck::FailedCall, 1)]),
            first_uncovered_s: Some(12.0),
            cps_ok_share: 1.0,
            cues: 10,
        }),
        ..FixRecord::default()
    }
}

fn corrections() -> Corrections {
    let fix = |id: &str| {
        correction(
            id,
            Chosen::FixIt {
                model: "opus".into(),
                why: "why".into(),
            },
        )
    };
    Corrections {
        lines: vec![
            fix("U1"),
            fix("U2"),
            fix("U7"),
            correction(
                "U4",
                Chosen::KeptFixIt {
                    model: "opus".into(),
                    why: "why".into(),
                },
            ),
            correction("U3", Chosen::Engine("adjudicated".into())),
            correction("U5", Chosen::Typed),
        ],
    }
}

#[test]
fn every_count_of_a_run() {
    let result = fix_result(&record(), &corrections(), &[]);
    assert_eq!(result.model, "Claude Opus");
    assert_eq!(result.changed, 5, "U1, U2, U3, U4 and U7 went in");
    assert_eq!(
        result.already_right, 3,
        "U5 as the owner corrected it, U6 turned down and U8 unchanged; U9 was never answered"
    );
    assert_eq!(result.turned_down, 1);
    assert_eq!(result.kept_yours, 1, "U5 keeps the owner's correction");
    assert_eq!((result.owner_kept, result.owner_undid), (1, 1));
}

#[test]
fn the_first_two_changes_in_id_order_are_the_examples() {
    let result = fix_result(&record(), &corrections(), &[]);
    assert_eq!(
        result.examples,
        [
            (String::new(), "Uh,".to_string()),
            ("Yeah,".to_string(), String::new()),
        ],
        "U1 puts a word in, U2 takes one out"
    );
    assert_eq!(result.more, 3);
}

#[test]
fn a_problem_is_cleared_when_none_of_its_kind_is_left() {
    let now = [Problem::FailedCall(2)];
    let result = fix_result(&record(), &corrections(), &now);
    assert_eq!(
        result.cleared,
        [Problem::Layout(2), Problem::UncoveredSpeech(12.0)],
        "the failed calls are still a problem, however many"
    );
    assert_eq!(result.left, now);
    let passing = fix_result(&record(), &corrections(), &[]);
    assert_eq!(passing.cleared.len(), 3);
    assert!(passing.left.is_empty());
}

#[test]
fn a_record_with_no_problems_before_clears_nothing() {
    let record = FixRecord {
        before: None,
        ..record()
    };
    let result = fix_result(&record, &corrections(), &[Problem::Offset]);
    assert!(result.cleared.is_empty());
    assert_eq!(result.left, [Problem::Offset]);
}

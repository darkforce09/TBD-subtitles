use job_model::outputs::{Chosen, Correction, FixBrief, FixVerdict, LineFix, Suspect};
use job_model::report::QcFinding;

use super::super::fake_model::{Fixture, GLOSSARY, finding};
use super::*;

/// A record line for `id`, asked about `checks`, with `verdict`.
fn record_line(id: &str, checks: &[QcCheck], verdict: FixVerdict) -> LineFix {
    LineFix {
        id: id.into(),
        problems: Vec::new(),
        checks: checks.to_vec(),
        before_text: "Oh?".into(),
        before_flags: Vec::new(),
        after_text: "Oh?".into(),
        after_flags: Vec::new(),
        steps: Vec::new(),
        refused: Vec::new(),
        removed: Vec::new(),
        verdict,
        applied: false,
    }
}

fn answered(lines: Vec<LineFix>) -> Answered {
    Answered::from_record(&FixRecord {
        lines,
        ..FixRecord::default()
    })
}

fn correction(id: &str, chosen: Chosen) -> Correction {
    Correction {
        id: id.into(),
        text: "Oh?".into(),
        flags: Vec::new(),
        chosen,
    }
}

fn fix_it() -> Chosen {
    Chosen::FixIt {
        model: "opus".into(),
        why: "x".into(),
    }
}

#[test]
fn each_check_goes_to_its_family_and_some_are_not_fix_it_s() {
    let none = Corrections::default();
    let nothing = Answered::none();
    let family = |check| asks_about(&finding(check, "U0314", 1.0, "Oh?", ""), &none, &nothing);
    assert_eq!(family(QcCheck::Unsure), Some(FixFamily::Words));
    assert_eq!(family(QcCheck::RemovedLocked), Some(FixFamily::Words));
    assert_eq!(family(QcCheck::TooShort), Some(FixFamily::Timing));
    assert_eq!(family(QcCheck::WeakTiming), Some(FixFamily::Timing));
    assert_eq!(family(QcCheck::GapTooSmall), Some(FixFamily::Timing));
    assert_eq!(family(QcCheck::TooFast), Some(FixFamily::ReadingSpeed));
    assert_eq!(family(QcCheck::Offset), None);
    assert_eq!(family(QcCheck::FailedCall), None);
    let sound_cue = QcFinding {
        utterance: None,
        ..finding(QcCheck::TooShort, "x", 1.0, "[door slams]", "12 frames")
    };
    assert_eq!(asks_about(&sound_cue, &none, &nothing), None);
}

#[test]
fn an_answered_line_covers_the_checks_it_was_asked_about_and_no_others() {
    let answered = answered(vec![
        record_line("U0314", &[QcCheck::TooShort], FixVerdict::Unchanged),
        record_line(
            "U0061",
            &[QcCheck::GapTooSmall],
            FixVerdict::NotAnswered {
                why: "rate limited".into(),
            },
        ),
        record_line(
            "U0062",
            &[QcCheck::Novel],
            FixVerdict::NotJudged { why: "x".into() },
        ),
    ]);
    let on = |check, id| answered.covers(&finding(check, id, 1.0, "", ""));
    assert!(on(QcCheck::TooShort, "U0314"));
    assert!(
        !on(QcCheck::TooFast, "U0314"),
        "a new check on the same line"
    );
    assert!(!on(QcCheck::GapTooSmall, "U0061"), "its calls failed");
    assert!(!on(QcCheck::Novel, "U0062"), "the judge gave no verdict");
    assert!(answered.line("U0314"));
    assert!(!answered.line("U0061"));
    let none = Answered::none();
    assert!(!none.covers(&finding(QcCheck::TooShort, "U0314", 1.0, "", "")));
    assert!(!none.line("U0314"));
}

#[test]
fn a_line_recorded_before_checks_were_kept_covers_every_check() {
    let answered = answered(vec![record_line(
        "U0314",
        &[],
        FixVerdict::Kept {
            why: "timed again".into(),
        },
    )]);
    for check in [QcCheck::TooShort, QcCheck::TooFast, QcCheck::Novel] {
        assert!(
            answered.covers(&finding(check, "U0314", 1.0, "", "")),
            "{check:?}"
        );
    }
}

#[test]
fn speech_with_no_subtitle_is_never_covered() {
    let answered = answered(vec![record_line(
        "U0295",
        &[QcCheck::UncoveredSpeech],
        FixVerdict::Unchanged,
    )]);
    let uncovered = finding(QcCheck::UncoveredSpeech, "U0295", 1103.2, "", "1.2 s");
    assert!(!answered.covers(&uncovered));
    let no_line = QcFinding {
        utterance: None,
        ..uncovered
    };
    assert!(!answered.covers(&no_line));
    assert_eq!(
        asks_about(&no_line, &Corrections::default(), &answered),
        Some(FixFamily::Timing)
    );
}

#[test]
fn answered_findings_are_not_asked_again() {
    let mut fixture = Fixture::new();
    fixture.answered = answered(vec![
        record_line("U0314", &[QcCheck::TooShort], FixVerdict::Unchanged),
        record_line("U0062", &[], FixVerdict::Unchanged),
    ]);
    let tiny = finding(QcCheck::TooShort, "U0314", 1156.1, "Oh?", "17 frames");
    assert_eq!(
        asks_about(&tiny, &Corrections::default(), &fixture.answered),
        None
    );
    let brief = FixBrief {
        suspects: vec![Suspect {
            id: "U0062".into(),
            why: "odd".into(),
        }],
        ..FixBrief::default()
    };
    let asked = items(&fixture.episode(&GLOSSARY), &brief);
    let ids: Vec<(&str, FixFamily)> = asked.iter().map(|i| (i.id.as_str(), i.family)).collect();
    // U0314 was answered about being too short, and U0062 as a suspect; the crowd line and the
    // lines next to speech with no subtitle are still asked.
    assert_eq!(
        ids,
        [
            ("U0061", FixFamily::Timing),
            ("U0294", FixFamily::Timing),
            ("U0295", FixFamily::Timing),
        ]
    );
    let checks = |id: &str| asked.iter().find(|i| i.id == id).unwrap().checks.clone();
    assert_eq!(checks("U0061"), [QcCheck::TooShort]);
    assert_eq!(checks("U0295"), [QcCheck::UncoveredSpeech]);
}

#[test]
fn the_owner_s_lines_are_left_alone_and_a_fix_is_not_asked_about_its_words_again() {
    let mut corrections = Corrections::default();
    corrections.set(correction("U0314", Chosen::Typed));
    corrections.set(correction("U0061", fix_it()));
    let on = |corrections: &Corrections, check, id| {
        asks_about(
            &finding(check, id, 1.0, "", ""),
            corrections,
            &Answered::none(),
        )
    };
    assert_eq!(on(&corrections, QcCheck::TooShort, "U0314"), None);
    assert_eq!(on(&corrections, QcCheck::Novel, "U0061"), None);
    assert_eq!(
        on(&corrections, QcCheck::TooShort, "U0061"),
        Some(FixFamily::Timing)
    );
    corrections.set(correction(
        "U0061",
        Chosen::KeptFixIt {
            model: "opus".into(),
            why: String::new(),
        },
    ));
    assert_eq!(on(&corrections, QcCheck::TooShort, "U0061"), None);
}

#[test]
fn speech_with_no_subtitle_asks_about_the_lines_next_to_it() {
    let fixture = Fixture::new();
    let ep = fixture.episode(&GLOSSARY);
    let asked = items(&ep, &FixBrief::default());
    let timing: Vec<&str> = asked
        .iter()
        .filter(|i| i.family == FixFamily::Timing)
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(timing, ["U0061", "U0294", "U0295", "U0314"]);
    let question = asked.iter().find(|i| i.id == "U0295").unwrap();
    assert_eq!(
        question.problems,
        ["speech from 18:23.2 to 18:24.4 has no subtitle; the main engine heard there: \"Uh\""]
    );
    assert!(question.keep_settles);
}

#[test]
fn the_brief_s_suspects_are_word_problems_unless_the_line_has_a_correction() {
    let mut fixture = Fixture::new();
    fixture.corrections.set(correction("U0060", fix_it()));
    let brief = FixBrief {
        suspects: vec![
            Suspect {
                id: "U0062".into(),
                why: "Luffy would not say this".into(),
            },
            Suspect {
                id: "U0060".into(),
                why: "odd".into(),
            },
        ],
        ..FixBrief::default()
    };
    let asked = items(&fixture.episode(&GLOSSARY), &brief);
    let words: Vec<&Item> = asked
        .iter()
        .filter(|i| i.family == FixFamily::Words)
        .collect();
    assert_eq!(words.len(), 1);
    assert_eq!(words[0].id, "U0062");
    assert_eq!(
        words[0].problems,
        ["does not fit the conversation: Luffy would not say this"]
    );
    assert!(!words[0].keep_settles);
}

#[test]
fn a_finding_reads_as_a_problem_in_words() {
    let short = finding(QcCheck::TooShort, "U0314", 1.0, "Oh?", "17 frames");
    assert_eq!(
        describe(&short),
        "its subtitle \"Oh?\" is on screen only 17 frames; it needs at least 20 frames"
    );
    let novel = finding(QcCheck::Novel, "U0062", 1.0, "", "U0062: Rebeca");
    assert_eq!(describe(&novel), "uses \"Rebeca\", which no engine heard");
    let uncovered = QcFinding {
        detail: "2.5 s".into(),
        ..finding(QcCheck::UncoveredSpeech, "x", 10.0, "", "")
    };
    assert_eq!(uncovered_span(&uncovered), (10.0, 12.5));
}

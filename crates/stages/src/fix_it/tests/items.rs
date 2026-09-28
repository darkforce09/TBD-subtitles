use job_model::outputs::{Chosen, Correction, FixBrief, Suspect};
use job_model::report::QcFinding;

use super::super::fake_model::{Fixture, GLOSSARY, finding};
use super::*;

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
    let family = |check| asks_about(&finding(check, "U0314", 1.0, "Oh?", ""), &none);
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
    assert_eq!(asks_about(&sound_cue, &none), None);
}

#[test]
fn the_owner_s_lines_are_left_alone_and_a_fix_is_not_asked_about_its_words_again() {
    let mut corrections = Corrections::default();
    corrections.set(correction("U0314", Chosen::Typed));
    corrections.set(correction("U0061", fix_it()));
    let on = |corrections: &Corrections, check, id| {
        asks_about(&finding(check, id, 1.0, "", ""), corrections)
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

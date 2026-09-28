use super::super::fake_model::{Fixture, GLOSSARY};
use super::*;

fn answer(t: &str, f: &[&str], why: &str) -> Answer {
    Answer {
        id: "U0295".into(),
        t: t.into(),
        f: f.iter().map(|s| s.to_string()).collect(),
        why: why.into(),
    }
}

const NOW: &str = "Does that mean you're gonna stay in this country?";

fn spk() -> Vec<String> {
    vec!["SPK".into()]
}

fn held(family: FixFamily, a: &Answer) -> Result<Outcome, String> {
    let fixture = Fixture::new();
    check(family, a, NOW, &spk(), &fixture.sheet, &GLOSSARY)
}

#[test]
fn a_heard_word_put_back_is_a_change() {
    let a = answer(
        "Uh, does that mean you're gonna stay in this country?",
        &["SPK"],
        "P heard Uh.",
    );
    assert_eq!(
        held(FixFamily::Timing, &a),
        Ok(Outcome::Change {
            text: a.t.clone(),
            flags: spk()
        })
    );
}

#[test]
fn the_line_as_it_is_is_a_keep() {
    let a = answer(NOW, &["SPK"], "Right as it is.");
    assert_eq!(held(FixFamily::Words, &a), Ok(Outcome::Keep));
}

#[test]
fn a_word_no_engine_heard_is_refused_but_a_glossary_name_is_not() {
    let invented = answer("Does that mean you're staying here?", &["SPK"], "Shorter.");
    let refused = held(FixFamily::Words, &invented).unwrap_err();
    assert!(
        refused.contains("\"staying\"") && refused.contains("\"here\""),
        "{refused}"
    );
    let named = answer(
        "Violet, does that mean you're gonna stay in this country?",
        &["SPK"],
        "She is named.",
    );
    assert!(matches!(
        held(FixFamily::Words, &named),
        Ok(Outcome::Change { .. })
    ));
}

#[test]
fn a_reason_known_flags_text_and_no_notation_are_required() {
    let no_reason = answer(
        "Uh, does that mean you're gonna stay in this country?",
        &["SPK"],
        " ",
    );
    assert_eq!(
        held(FixFamily::Timing, &no_reason).unwrap_err(),
        "gave no reason"
    );
    let unsure = answer(NOW, &["UNSURE"], "Cannot tell.");
    assert!(
        held(FixFamily::Words, &unsure)
            .unwrap_err()
            .contains("UNSURE")
    );
    let empty = answer("", &["SPK"], "Noise.");
    assert!(
        held(FixFamily::Timing, &empty)
            .unwrap_err()
            .contains("without DROP")
    );
    let dropped = answer("", &["DROP"], "Noise.");
    assert!(matches!(
        held(FixFamily::Timing, &dropped),
        Ok(Outcome::Change { .. })
    ));
    let notation = answer(
        "{P:Uh|W:∅} does that mean you're gonna stay in this country?",
        &["SPK"],
        "x",
    );
    assert!(
        held(FixFamily::Words, &notation)
            .unwrap_err()
            .contains("notation")
    );
    let speakers = answer(
        "Does that mean you're gonna stay? || In this country?",
        &["SPK"],
        "Two.",
    );
    assert!(matches!(
        held(FixFamily::Words, &speakers),
        Ok(Outcome::Change { .. })
    ));
}

#[test]
fn a_reading_speed_fix_only_takes_words_out_and_keeps_the_flags() {
    let shorter = answer("Does that mean you're gonna stay?", &["SPK"], "Shorter.");
    assert!(matches!(
        held(FixFamily::ReadingSpeed, &shorter),
        Ok(Outcome::Change { .. })
    ));
    let reordered = answer("You're gonna stay in this country?", &["SPK"], "Shorter.");
    assert!(matches!(
        held(FixFamily::ReadingSpeed, &reordered),
        Ok(Outcome::Change { .. })
    ));
    let added = answer("Uh, does that mean you're gonna stay?", &["SPK"], "x");
    assert!(
        held(FixFamily::ReadingSpeed, &added)
            .unwrap_err()
            .contains("added or replaced")
    );
    let flagged = answer("Does that mean you're gonna stay?", &[], "x");
    assert!(
        held(FixFamily::ReadingSpeed, &flagged)
            .unwrap_err()
            .contains("flags")
    );
}

#[test]
fn flags_come_back_in_one_order_without_unsure() {
    let flags: Vec<String> = ["DROP", "UNSURE", "NARR", "DROP"]
        .map(String::from)
        .to_vec();
    assert_eq!(
        ordered(&flags),
        vec!["NARR".to_string(), "DROP".to_string()]
    );
}

#[test]
fn the_heard_words_a_change_leaves_out_are_listed() {
    assert_eq!(
        removed("I... I said no, no!", "I said no!"),
        vec!["I".to_string(), "no!".to_string()]
    );
    assert!(removed("Shut up!", "Shut up!").is_empty());
    assert!(only_takes_out("well, I said no", "I said no"));
    assert!(!only_takes_out("I said no", "no I said"));
}

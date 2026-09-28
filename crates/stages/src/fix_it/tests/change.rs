use job_model::outputs::FixVerdict;

use super::*;

fn change(before: &str, after: &str) -> Option<(String, String)> {
    changed_words(&LineFix {
        id: "U0295".into(),
        problems: Vec::new(),
        checks: Vec::new(),
        before_text: before.into(),
        before_flags: Vec::new(),
        after_text: after.into(),
        after_flags: Vec::new(),
        steps: Vec::new(),
        refused: Vec::new(),
        removed: Vec::new(),
        verdict: FixVerdict::Unchanged,
        applied: false,
    })
}

fn pair(was: &str, is: &str) -> Option<(String, String)> {
    Some((was.into(), is.into()))
}

#[test]
fn a_word_put_in_is_named_alone() {
    assert_eq!(
        change(
            "Does that mean you're gonna stay?",
            "Uh, does that mean you're gonna stay?"
        ),
        pair("", "Uh,")
    );
}

#[test]
fn a_word_taken_out_is_named_alone() {
    assert_eq!(
        change("Yeah, shut your filthy mouths!", "Shut your filthy mouths!"),
        pair("Yeah,", "")
    );
}

#[test]
fn replaced_words_are_named_as_written() {
    assert_eq!(
        change("Look at Rebeca, now!", "Look at Rebecca, now!"),
        pair("Rebeca,", "Rebecca,")
    );
    assert_eq!(
        change("I want the red one here.", "I want a blue bag here."),
        pair("the red one", "a blue bag")
    );
}

#[test]
fn only_the_first_run_of_changed_words_is_named() {
    assert_eq!(
        change("Uh go to the port now", "go to a port now please"),
        pair("Uh", "")
    );
}

#[test]
fn a_change_of_punctuation_alone_is_found() {
    assert_eq!(change("Go! Now!", "Go. Now!"), pair("Go!", "Go."));
}

#[test]
fn the_same_words_are_no_change() {
    assert_eq!(change("Oh?", "Oh?"), None);
    assert_eq!(change("Oh?  Panties?", "Oh? Panties?"), None);
    assert_eq!(change("", ""), None);
}

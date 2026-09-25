use super::*;

fn problems(path: &str, text: &str) -> Vec<String> {
    ProseRules::new().problems(path, text.as_bytes())
}

/// A history word assembled at run time, so this file holds none whole.
fn word(index: usize) -> String {
    let (head, tail) = HISTORY_WORDS[index];
    format!("{head}{tail}")
}

#[test]
fn every_rule_fires_on_a_line_that_breaks_it() {
    let text = format!(
        "// See T-104.\n// Part of M0.5.\n// This was {} slow.\n",
        word(0)
    );
    assert_eq!(
        problems("crates/x/src/a.rs", &text),
        [
            "crates/x/src/a.rs:1: ticket id `T-104`",
            "crates/x/src/a.rs:2: milestone id `M0.5`; the roadmap tracks milestones",
            format!(
                "crates/x/src/a.rs:3: history word `{}`; say what is, and leave the past to git",
                word(0)
            )
            .as_str(),
        ]
    );
}

#[test]
fn every_history_word_is_caught_in_any_case() {
    for index in 0..HISTORY_WORDS.len() {
        let spelled = word(index).replace("(?:from|to)", "from").to_uppercase();
        let found = problems(
            "documentation/guide.md",
            &format!("It is {spelled} here.\n"),
        );
        assert_eq!(found.len(), 1, "{spelled}");
    }
}

#[test]
fn milestones_are_allowed_in_documents_and_history_in_the_decision_log() {
    assert!(problems("documentation/roadmap.md", "## M0 — Workspace\n").is_empty());
    assert!(problems(PROJECT_INSTRUCTIONS, "Next step: M0.\n").is_empty());
    let text = format!("The choice {} made is replaced.\n", word(1));
    assert!(problems("documentation/decisions.md", &text).is_empty());
    assert_eq!(problems("crates/x/README.md", "Built in M1.\n").len(), 1);
}

#[test]
fn frozen_records_and_tests_are_not_judged() {
    let rule = ProseRules::new();
    assert!(!rule.selects("documentation/research/snapshot.md"));
    assert!(rule.selects("documentation/research/README.md"));
    assert!(!rule.selects("crates/x/src/tests/a.rs"));
    assert!(!rule.selects("crates/x/Cargo.toml"));
}

#[test]
fn a_word_inside_another_word_is_not_a_history_word() {
    assert!(
        problems(
            "crates/x/src/a.rs",
            "// The format erlang uses.\n// MM0 T-1\n"
        )
        .is_empty()
    );
}

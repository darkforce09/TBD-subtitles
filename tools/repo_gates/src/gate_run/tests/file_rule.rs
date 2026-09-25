use super::*;
use crate::gate_run::UntrackedFiles;
use crate::gate_run::fixture_checkout::{FixtureCheckout, failures, outcome_counts, request};

/// A rule over `.txt` files that finds every line saying `bad`.
struct NoBadLines;

impl FileRule for NoBadLines {
    fn gate(&self) -> &'static str {
        "no-bad-lines"
    }

    fn describes(&self) -> String {
        "no line says bad".to_string()
    }

    fn selects(&self, path: &str) -> bool {
        extension(path) == Some("txt")
    }

    fn problems(&self, path: &str, bytes: &[u8]) -> Vec<String> {
        String::from_utf8_lossy(bytes)
            .lines()
            .enumerate()
            .filter(|(_, line)| *line == "bad")
            .map(|(index, _)| format!("{path}:{}: says bad", index + 1))
            .collect()
    }
}

fn run(fixture: &FixtureCheckout, scope: &[&str]) -> GateRun {
    judge_file_rule(
        &NoBadLines,
        fixture.root(),
        Ok(fixture.tree()),
        &request(scope, UntrackedFiles::Invisible),
    )
}

#[test]
fn every_selected_file_gets_one_verdict_with_its_problems() {
    let mut fixture = FixtureCheckout::new("file-rule-verdicts");
    fixture
        .tracked("a/good.txt", "fine\n")
        .tracked("a/bad.txt", "fine\nbad\nbad\n")
        .tracked("a/other.rs", "bad\n");
    let run = run(&fixture, &[]);
    assert_eq!(
        failures(&run),
        ["FAIL: a/bad.txt: 2 problem(s)\n      a/bad.txt:2: says bad\n      a/bad.txt:3: says bad"]
    );
    assert_eq!(outcome_counts(&run), (1, 1, 0));
    assert_eq!(
        run.totals,
        ["  2 file(s) judged, 1 with problems, 0 unreadable"]
    );
    assert_eq!(run.print(), 1);
}

#[test]
fn an_unreadable_file_or_nothing_selected_did_not_run() {
    let mut fixture = FixtureCheckout::new("file-rule-unreadable");
    fixture.listed_only("a/gone.txt").tracked("b/other.rs", "");
    let unreadable = run(&fixture, &["a"]);
    assert_eq!(outcome_counts(&unreadable), (0, 0, 1));
    let nothing = run(&fixture, &["b"]);
    assert!(failures(&nothing)[0].starts_with("FAIL: no-bad-lines judged nothing in b"));
    assert_eq!(nothing.print(), 2);
}

#[test]
fn test_paths_and_extensions_are_read_from_the_path() {
    assert!(is_test_path("crates/x/src/tests/case.rs"));
    assert!(!is_test_path("crates/x/src/latests.rs"));
    assert!(!is_test_path("crates/tests"));
    assert_eq!(extension("a/b.rs"), Some("rs"));
    assert_eq!(extension("a/.gitignore"), None);
    assert_eq!(extension("a/Makefile"), None);
}

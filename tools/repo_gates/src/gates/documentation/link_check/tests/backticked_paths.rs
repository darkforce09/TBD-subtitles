use std::cell::RefCell;

use verification_core::NotRun;

use super::super::markdown_scan::scan;
use super::super::{Break, BreakListing, judge as judge_gate};
use super::*;
use crate::gate_run::GateRequest;
use crate::gate_run::fixture_checkout::{FixtureCheckout, failures, outcome_counts};
use crate::layout::FROZEN_FOLDERS;

/// The top-level folders of the fixture checkouts below.
fn top_level(first: &str) -> bool {
    ["apps", "tools", "crates", ".cargo"].contains(&first)
}

fn path(path: &str, folder: bool) -> CodeSpanReading {
    CodeSpanReading::Path(CitedPath {
        path: Some(path.to_string()),
        folder,
    })
}

/// An ignore-rules source with a fixed answer, recording every batch it is asked.
#[derive(Default)]
struct FakeIgnoreRules {
    ignored: BTreeSet<String>,
    fail: bool,
    batches: RefCell<Vec<Vec<String>>>,
}

impl FakeIgnoreRules {
    fn ignoring(paths: &[&str]) -> FakeIgnoreRules {
        FakeIgnoreRules {
            ignored: paths.iter().map(ToString::to_string).collect(),
            ..FakeIgnoreRules::default()
        }
    }
}

impl IgnoreRules for FakeIgnoreRules {
    fn ignored(&self, paths: &[String]) -> Result<BTreeSet<String>, NotRun> {
        self.batches.borrow_mut().push(paths.to_vec());
        if self.fail {
            return Err(NotRun::ToolError {
                tool: "git check-ignore --stdin -z".to_string(),
                status: 128,
                stderr: "fatal".to_string(),
            });
        }
        Ok(paths
            .iter()
            .filter(|path| self.ignored.contains(*path))
            .cloned()
            .collect())
    }
}

/// A checkout whose top-level folders are `apps`, `crates` and `tools`, with a tracked
/// source file and two READMEs.
fn checkout(tag: &str) -> FixtureCheckout {
    let mut fixture = FixtureCheckout::new(&format!("backticked-{tag}"));
    fixture
        .tracked("apps/tool/src/main.rs", "fn main() {}\n")
        .tracked("apps/tool/README.md", "# Tool\n")
        .tracked("crates/definitions/mission.schema.json", "{}\n")
        .tracked("tools/README.md", "# Tools\n");
    fixture
}

/// What the rule makes of `documents` (path, text) written into `fixture` as tracked: every
/// break as it renders, how many checks did not run, and the totals line.
fn judge_documents(
    fixture: &mut FixtureCheckout,
    documents: &[(&str, &str)],
    ignore_rules: &FakeIgnoreRules,
) -> (Vec<String>, usize, String) {
    for (document, text) in documents {
        fixture.tracked(document, text);
    }
    let tree = fixture.tree();
    let context = RuleContext {
        repo_root: fixture.root(),
        tree: &tree,
    };
    let mut rule = BacktickedPaths::new(ignore_rules);
    let mut findings = RuleFindings::default();
    for (document, text) in documents {
        let scanned = scan(text);
        let judged = JudgedDocument {
            path: document,
            scan: &scanned,
        };
        rule.judge(&judged, &context, &mut findings);
    }
    rule.finish(&context, &mut findings);
    let breaks = findings.breaks.iter().map(Break::render).collect();
    (breaks, findings.not_run.len(), rule.totals().join("\n"))
}

#[test]
fn only_a_span_under_a_top_level_folder_is_a_repository_path() {
    for text in [
        "README.md",
        "src/main.rs",
        "https://example.com/apps/x",
        "/apps/tool",
        "./apps/tool",
        "cargo gates link-check",
        "documentation/guide.md",
    ] {
        assert_eq!(
            read_code_span(text, top_level),
            CodeSpanReading::NotARepositoryPath,
            "{text}"
        );
    }
    assert_eq!(
        read_code_span(" apps/tool/README.md ", top_level),
        path("apps/tool/README.md", false)
    );
}

#[test]
fn every_pattern_class_is_skipped() {
    for (text, kind) in [
        ("apps/*.rs", PatternKind::Glob),
        ("apps/tool/?.rs", PatternKind::Glob),
        ("apps/[ab]/mod.rs", PatternKind::Glob),
        ("apps/<page>/mod.rs", PatternKind::Placeholder),
        (".cargo/<name>.toml", PatternKind::Placeholder),
        ("crates/{definitions,rules}", PatternKind::Set),
        ("apps/$APP/README.md", PatternKind::Variable),
        ("apps/tool --release", PatternKind::Command),
        ("apps/tool/x\ty", PatternKind::Command),
        ("apps/https://example.com/x?y=1", PatternKind::Url),
        ("apps/website/...", PatternKind::Elision),
        ("apps/…/mod.rs", PatternKind::Elision),
    ] {
        assert_eq!(
            read_code_span(text, top_level),
            CodeSpanReading::Pattern(kind),
            "{text}"
        );
    }
}

#[test]
fn a_line_suffix_and_a_fragment_are_stripped_before_the_check() {
    for (text, expected) in [
        ("apps/tool/src/main.rs:12", "apps/tool/src/main.rs"),
        ("apps/tool/src/main.rs:12-40", "apps/tool/src/main.rs"),
        ("apps/tool/src/main.rs:12:5", "apps/tool/src/main.rs"),
        (
            "crates/definitions/mission.schema.json#/definitions/Slot",
            "crates/definitions/mission.schema.json",
        ),
        ("apps/tool/README.md#usage", "apps/tool/README.md"),
    ] {
        assert_eq!(
            read_code_span(text, top_level),
            path(expected, false),
            "{text}"
        );
    }
    for text in [
        "apps/tool/main.rs:main",
        "apps/tool/main.rs:12-",
        "apps/tool/x:1:2:3",
    ] {
        assert_eq!(
            read_code_span(text, top_level),
            path(text, false),
            "{text} keeps a suffix that is no line range"
        );
    }
}

#[test]
fn a_trailing_slash_asks_for_a_folder_and_segments_are_normalised() {
    assert_eq!(
        read_code_span("apps/tool/", top_level),
        path("apps/tool", true)
    );
    assert_eq!(
        read_code_span("apps/tool", top_level),
        path("apps/tool", false)
    );
    assert_eq!(
        read_code_span("apps//tool/./src/../README.md", top_level),
        path("apps/tool/README.md", false)
    );
    assert_eq!(
        read_code_span("apps/../../outside", top_level),
        CodeSpanReading::Path(CitedPath {
            path: None,
            folder: false
        })
    );
}

#[test]
fn a_tracked_file_or_folder_passes_without_asking_git() {
    let mut fixture = checkout("tracked");
    let ignore_rules = FakeIgnoreRules::default();
    let text = "# Doc\n\n`apps/tool/src/main.rs:1` `apps/tool` `apps/tool/` `apps/tool/README.md` \
                `crates/definitions/mission.schema.json#/definitions/Slot`\n";
    let (breaks, not_run, totals) =
        judge_documents(&mut fixture, &[("apps/tool/guide.md", text)], &ignore_rules);
    assert!(breaks.is_empty(), "{breaks:?}");
    assert_eq!(not_run, 0);
    assert!(
        ignore_rules.batches.borrow().is_empty(),
        "nothing waits for git"
    );
    assert!(
        totals.contains("5 judged in live documents — 5 tracked"),
        "{totals}"
    );
}

#[test]
fn a_path_naming_nothing_breaks_and_a_file_is_no_folder() {
    let mut fixture = checkout("nothing");
    let text = "# Doc\n\nSee `apps/tool/src/gone.rs:4`.\n\n`apps/tool/src/main.rs/` and \
                `apps/../../outside`.\n";
    let (breaks, not_run, _) = judge_documents(
        &mut fixture,
        &[("apps/tool/README.md", text)],
        &FakeIgnoreRules::default(),
    );
    assert_eq!(not_run, 0);
    assert_eq!(
        breaks,
        [
            "apps/tool/README.md:5: backticked path names nothing: `apps/../../outside` climbs \
             above the repository root",
            "apps/tool/README.md:3: backticked path names nothing: `apps/tool/src/gone.rs:4` \
             (checked as `apps/tool/src/gone.rs`)",
            "apps/tool/README.md:5: backticked path names nothing: `apps/tool/src/main.rs/`",
        ]
    );
}

#[test]
fn every_waiting_path_is_asked_in_one_batch_and_an_ignored_one_passes() {
    let mut fixture = checkout("ignored");
    let ignore_rules = FakeIgnoreRules::ignoring(&[
        "apps/tool/target/debug/tool",
        "tools/deploy/deploy.env",
        "apps/tool/dist/",
    ]);
    let first = "# First\n\n`apps/tool/target/debug/tool` `apps/tool/dist` `apps/tool/gone.md`\n";
    let second = "# Second\n\n`tools/deploy/deploy.env` `apps/tool/gone.md` `apps/gone/`\n";
    let (breaks, not_run, totals) = judge_documents(
        &mut fixture,
        &[("apps/tool/first.md", first), ("crates/second.md", second)],
        &ignore_rules,
    );
    assert_eq!(not_run, 0);
    assert_eq!(
        breaks,
        [
            "apps/tool/first.md:3: backticked path names nothing: `apps/tool/gone.md`",
            "crates/second.md:3: backticked path names nothing: `apps/tool/gone.md`",
            "crates/second.md:3: backticked path names nothing: `apps/gone/`",
        ]
    );
    assert_eq!(
        *ignore_rules.batches.borrow(),
        [vec![
            "apps/gone/".to_string(),
            "apps/tool/dist".to_string(),
            "apps/tool/dist/".to_string(),
            "apps/tool/gone.md".to_string(),
            "apps/tool/gone.md/".to_string(),
            "apps/tool/target/debug/tool".to_string(),
            "apps/tool/target/debug/tool/".to_string(),
            "tools/deploy/deploy.env".to_string(),
            "tools/deploy/deploy.env/".to_string(),
        ]],
        "one batch, each distinct question once; a span without `/` is asked as a folder too"
    );
    assert!(
        totals.contains("6 judged in live documents — 0 tracked, 3 ignored by git"),
        "{totals}"
    );
}

#[test]
fn a_failed_batch_did_not_run_and_breaks_nothing() {
    let mut fixture = checkout("failed");
    let ignore_rules = FakeIgnoreRules {
        fail: true,
        ..FakeIgnoreRules::default()
    };
    let (breaks, not_run, totals) = judge_documents(
        &mut fixture,
        &[("apps/tool/README.md", "`apps/gone.md` `apps/tool/`\n")],
        &ignore_rules,
    );
    assert!(breaks.is_empty());
    assert_eq!(not_run, 1);
    assert!(totals.contains("1 tracked"), "{totals}");
    assert!(totals.contains("1 unchecked"), "{totals}");
}

#[test]
fn fenced_code_is_not_read_for_paths_and_patterns_are_counted() {
    let mut fixture = checkout("fenced");
    let text = "```text\napps/gone.md\n```\n\n`apps/*.md` `apps/<x>`\n";
    let (breaks, _, totals) = judge_documents(
        &mut fixture,
        &[("apps/tool/README.md", text)],
        &FakeIgnoreRules::default(),
    );
    assert!(breaks.is_empty(), "{breaks:?}");
    assert!(
        totals.ends_with("0 unchecked; 2 pattern(s) skipped"),
        "{totals}"
    );
}

#[test]
fn frozen_records_are_never_judged_by_the_rule() {
    let mut fixture = checkout("frozen");
    let stale = "# Record\n\nSee `apps/tool/gone.rs`.\n";
    fixture
        .tracked(&format!("{}/record.md", FROZEN_FOLDERS[0]), stale)
        .tracked("apps/tool/guide/README.md", stale);
    let ignore_rules = FakeIgnoreRules::default();
    let mut rules: Vec<Box<dyn DocumentRule + '_>> =
        vec![Box::new(BacktickedPaths::new(&ignore_rules))];
    let run = judge_gate(
        fixture.root(),
        Ok(fixture.tree()),
        &GateRequest::default(),
        BreakListing::Every,
        &mut rules,
    );
    assert_eq!(
        failures(&run)
            .iter()
            .map(|failure| failure.lines().next().unwrap_or("").to_string())
            .collect::<Vec<_>>(),
        ["FAIL: apps/tool/guide/README.md: 1 break(s)"]
    );
    assert_eq!(outcome_counts(&run), (3, 1, 0));
    assert!(
        run.totals
            .contains(&"    backticked path names nothing: 1".to_string())
    );
}

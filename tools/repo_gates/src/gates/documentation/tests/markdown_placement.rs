use super::*;
use crate::gate_run::UntrackedFiles;
use crate::gate_run::fixture_checkout::{FixtureCheckout, failures, outcome_counts, request};
use crate::layout::FROZEN_FOLDERS;

fn run(fixture: &FixtureCheckout, scope: &[&str]) -> GateRun {
    judge(
        fixture.root(),
        Ok(fixture.tree()),
        &request(scope, UntrackedFiles::Invisible),
    )
}

fn lines(count: usize) -> String {
    "a line\n".repeat(count)
}

#[test]
fn code_trees_hold_only_readme_markdown() {
    let mut fixture = FixtureCheckout::new("placement-code");
    fixture
        .tracked("apps/README.md", "# Apps\n")
        .tracked("apps/tool/NOTES.md", "# Notes\n")
        .tracked("apps/tool/rules.MD", "# Rules\n")
        .tracked("apps/tool/tests/fixture.md", "")
        .tracked("apps/tool/generated/api.md", "")
        .tracked("apps/.cursor/guide.md", "")
        .tracked("apps/tool/main.rs", "");
    let run = run(&fixture, &[]);
    assert_eq!(
        failures(&run),
        [
            "FAIL: apps/tool/NOTES.md: Markdown in a code tree; a code tree holds only README.md, \
             and documents live under documentation/",
            "FAIL: apps/tool/rules.MD: Markdown in a code tree; a code tree holds only README.md, \
             and documents live under documentation/",
        ]
    );
    assert_eq!(outcome_counts(&run), (1, 2, 0), "README.md held");
    assert_eq!(
        run.totals[0],
        "  code trees: 3 Markdown file(s) judged, 2 other than README.md"
    );
    assert_eq!(run.print(), 1);
}

#[test]
fn live_documents_stay_at_or_under_the_limit() {
    let mut fixture = FixtureCheckout::new("placement-size");
    fixture
        .tracked("documentation/runbooks/short.md", &lines(500))
        .tracked("documentation/runbooks/long.md", &lines(501))
        .tracked("documentation/runbooks/table.csv", &lines(900));
    let run = run(&fixture, &[]);
    assert_eq!(
        failures(&run),
        [
            "FAIL: documentation/runbooks/long.md: 501 lines; a live document stays at or under \
          500, so split it by topic into a folder with a README.md index"
        ]
    );
    assert_eq!(
        run.totals[1],
        "  documentation/: 2 live document(s) judged, 1 over 500 lines, 0 unreadable"
    );
}

#[test]
fn frozen_records_are_outside_the_limit() {
    let mut fixture = FixtureCheckout::new("placement-exempt");
    let long = lines(900);
    for exempt in FROZEN_FOLDERS
        .iter()
        .map(|folder| format!("{folder}/snapshot.md"))
    {
        fixture.tracked(&exempt, &long);
    }
    fixture.tracked("documentation/README.md", &lines(10));
    let run = run(&fixture, &[]);
    assert_eq!(failures(&run), Vec::<String>::new());
    assert_eq!(
        outcome_counts(&run),
        (1, 0, 0),
        "only the root README is judged"
    );
}

#[test]
fn a_document_missing_from_the_disk_did_not_run() {
    let mut fixture = FixtureCheckout::new("placement-unreadable");
    fixture.listed_only("documentation/runbooks/gone.md");
    let run = run(&fixture, &[]);
    assert_eq!(outcome_counts(&run), (0, 0, 1));
    assert_eq!(
        run.totals[1],
        "  documentation/: 1 live document(s) judged, 0 over 500 lines, 1 unreadable"
    );
    assert_eq!(run.print(), 2);
}

#[test]
fn the_scope_narrows_every_rule() {
    let mut fixture = FixtureCheckout::new("placement-scope");
    fixture
        .tracked("apps/tool/NOTES.md", "# Notes\n")
        .tracked("documentation/long.md", &lines(600))
        .tracked("documentation/area/short.md", &lines(5));
    let documentation = run(&fixture, &["documentation/area"]);
    assert_eq!(outcome_counts(&documentation), (1, 0, 0));
    let code = run(&fixture, &["apps"]);
    assert_eq!(outcome_counts(&code), (0, 1, 0));
    let whole = run(&fixture, &[]);
    assert_eq!(outcome_counts(&whole), (1, 2, 0));
}

#[test]
fn untracked_markdown_is_placed_only_with_untracked_files_included() {
    let mut fixture = FixtureCheckout::new("placement-untracked");
    fixture
        .tracked(".gitignore", "build/\n")
        .tracked("apps/README.md", "# Apps\n")
        .untracked("apps/tool/NOTES.md", "# Notes\n")
        .untracked("apps/tool/build/report.md", "# Report\n")
        .untracked("documentation/runbooks/long.md", &lines(501));
    let run_listed_by_git = |untracked| {
        judge(
            fixture.root(),
            fixture.listed_by_git(untracked),
            &request(&[], untracked),
        )
    };
    let committed = run_listed_by_git(UntrackedFiles::Invisible);
    assert_eq!(failures(&committed), Vec::<String>::new());
    assert_eq!(committed.summary_label(), "markdown-placement");

    let with_untracked = run_listed_by_git(UntrackedFiles::Included);
    assert_eq!(
        failures(&with_untracked),
        [
            "FAIL: apps/tool/NOTES.md: Markdown in a code tree; a code tree holds only README.md, \
             and documents live under documentation/",
            "FAIL: documentation/runbooks/long.md: 501 lines; a live document stays at or \
             under 500, so split it by topic into a folder with a README.md index",
        ],
        "the ignored build/ report is never judged"
    );
    assert_eq!(
        with_untracked.summary_label(),
        "markdown-placement --with-untracked (untracked files included)"
    );
    assert_eq!(with_untracked.print(), 1);
}

#[test]
fn a_failed_listing_or_an_empty_scope_did_not_run() {
    let failed = judge(
        Path::new("/nonexistent/documentation-gates"),
        Err(NotRun::ToolError {
            tool: "git ls-files -z".to_string(),
            status: 128,
            stderr: "fatal: not a git repository".to_string(),
        }),
        &GateRequest::default(),
    );
    assert_eq!(
        failures(&failed)[0].lines().next(),
        Some(
            "FAIL: markdown-placement could not list the tracked files — git ls-files -z exited 128"
        )
    );
    assert_eq!(failed.print(), 2);

    let mut fixture = FixtureCheckout::new("placement-empty-scope");
    fixture
        .tracked("apps/README.md", "# Apps\n")
        .tracked(".ai/tickets/ROOT", "");
    let outside = run(&fixture, &[".ai"]);
    assert_eq!(outcome_counts(&outside), (0, 0, 1));
    assert!(failures(&outside)[0].starts_with("FAIL: markdown-placement judged nothing in .ai"));
    assert_eq!(outside.print(), 2);
}

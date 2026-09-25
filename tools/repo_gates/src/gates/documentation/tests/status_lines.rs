use super::*;

fn problems(path: &str, text: &str) -> Vec<String> {
    StatusLines::new().problems(path, text.as_bytes())
}

#[test]
fn the_three_statuses_pass() {
    for first in [
        "**Status:** live",
        "**Status:** frozen record (2026-09-25)",
        "**Status:** archived",
    ] {
        assert!(
            problems("documentation/x.md", &format!("{first}\n\n# X\n")).is_empty(),
            "{first}"
        );
    }
}

#[test]
fn a_missing_misspelled_or_undated_status_fails() {
    for text in [
        "# X\n",
        "**Status:** Live\n",
        "**Status:** frozen record\n",
        "\n**Status:** live\n",
        "",
    ] {
        assert_eq!(problems("documentation/x.md", text).len(), 1, "{text:?}");
    }
}

#[test]
fn a_frozen_folder_holds_no_live_document_but_its_index() {
    assert_eq!(
        problems("documentation/research/stack.md", "**Status:** live\n"),
        [
            "documentation/research/stack.md:1: a document in a frozen folder is a frozen record or \
          archived"
        ]
    );
    assert!(problems("documentation/research/README.md", "**Status:** live\n").is_empty());
}

#[test]
fn only_markdown_under_the_documentation_root_is_judged() {
    let rule = StatusLines::new();
    assert!(rule.selects("documentation/a/b.md"));
    assert!(!rule.selects("crates/x/README.md"));
    assert!(!rule.selects("documentation/a/data.json"));
}

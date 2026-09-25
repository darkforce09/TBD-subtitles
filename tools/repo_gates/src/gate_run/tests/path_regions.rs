use super::*;
use crate::layout::{CODE_TREES, DOCUMENTATION_ROOT, FROZEN_FOLDERS};

#[test]
fn a_path_is_within_a_folder_only_at_a_component_boundary() {
    assert!(is_within("apps", "apps"));
    assert!(is_within("apps/a/b.rs", "apps"));
    assert!(is_within("apps/a/b.rs", "apps/a"));
    assert!(!is_within("apps_extra/x", "apps"));
    assert!(!is_within("app", "apps"));
    assert!(
        is_within("anything", ""),
        "the repository root holds every path"
    );
}

#[test]
fn paths_split_into_folder_and_name() {
    assert_eq!(parent_folder("apps/a/b.rs"), "apps/a");
    assert_eq!(parent_folder("top.md"), "");
    assert_eq!(file_name("apps/a/b.rs"), "b.rs");
    assert_eq!(file_name("top.md"), "top.md");
    assert_eq!(join("apps/a", README), "apps/a/README.md");
    assert_eq!(join("", README), "README.md");
}

#[test]
fn the_readme_span_is_the_code_trees_and_the_documentation_root() {
    for tree in CODE_TREES {
        assert!(in_code_tree(tree));
        assert!(in_readme_span(tree));
        assert!(in_readme_span(&format!("{tree}/deep/folder")));
    }
    assert!(in_readme_span(DOCUMENTATION_ROOT));
    assert!(in_readme_span(FROZEN_FOLDERS[0]));
    assert!(!in_code_tree(DOCUMENTATION_ROOT));
    assert!(!in_readme_span(".ai/tickets"));
    assert!(!in_readme_span(""));
}

#[test]
fn test_generated_and_hidden_folders_are_exempt_with_their_subtrees() {
    assert!(below_exempt_folder("apps/x/tests"));
    assert!(below_exempt_folder("apps/x/tests/fixtures"));
    assert!(below_exempt_folder("apps/x/generated/models"));
    assert!(below_exempt_folder("apps/mod/.cursor/rules"));
    assert!(!below_exempt_folder("apps/x/test_fixtures"));
    assert!(!below_exempt_folder("apps/x/latests"));
    assert!(!below_exempt_folder("apps/x/src"));
}

#[test]
fn exempt_folders_lie_outside_the_readme_span() {
    for exempt in [
        "apps/x/tests",
        "apps/x/tests/fixtures",
        "tools/x/generated",
        "apps/x/.cfg",
        "apps/x/.cfg/nested",
    ] {
        assert!(!in_readme_span(exempt), "{exempt} is exempt");
    }
    let tests_below_documentation = format!("{DOCUMENTATION_ROOT}/topic/tests");
    assert!(!in_readme_span(&tests_below_documentation));
    assert!(in_readme_span("apps/x/test_fixtures"));
    assert!(in_readme_span(&format!("{DOCUMENTATION_ROOT}/topic")));
}

#[test]
fn markdown_is_any_letter_case_of_the_md_extension() {
    assert!(is_markdown("apps/a/NOTES.md"));
    assert!(is_markdown("apps/a/NOTES.MD"));
    assert!(is_markdown("README.md"));
    assert!(!is_markdown("apps/a/rules.mdc"));
    assert!(!is_markdown("apps/a/md"));
    assert!(!is_markdown("apps/a/.md"));
}

#[test]
fn the_size_limit_skips_frozen_records_only() {
    for folder in FROZEN_FOLDERS {
        let exempt = format!("{folder}/snapshot.md");
        assert!(is_size_exempt(&exempt), "{exempt} should be exempt");
        assert!(is_frozen_record(&exempt));
        let index = format!("{folder}/README.md");
        assert!(!is_size_exempt(&index), "{index} is a live index");
    }
    for live in [
        format!("{DOCUMENTATION_ROOT}/README.md"),
        format!("{DOCUMENTATION_ROOT}/runbooks/deploy.md"),
    ] {
        assert!(!is_size_exempt(&live), "{live} is a live document");
    }
}

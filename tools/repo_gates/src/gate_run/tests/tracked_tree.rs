use super::*;
use crate::gate_run::fixture_checkout::FixtureCheckout;
use std::collections::BTreeSet;
use verification_core::NotRun;
use verification_core::proc::Output;

fn names(set: &BTreeSet<String>) -> Vec<&str> {
    set.iter().map(String::as_str).collect()
}

fn failed_output() -> Output {
    Output {
        code: 128,
        stdout: String::new(),
        stderr: "fatal: not a git repository\n".to_string(),
        duration: std::time::Duration::ZERO,
    }
}

#[test]
fn a_listing_yields_its_files_and_every_folder_above_them() {
    let tree = TrackedTree::from_listing("apps/a/b/c.rs\0apps/a/README.md\0top.md\0");
    assert_eq!(
        tree.files().collect::<Vec<_>>(),
        ["apps/a/README.md", "apps/a/b/c.rs", "top.md"]
    );
    assert_eq!(
        tree.folders().collect::<Vec<_>>(),
        ["apps", "apps/a", "apps/a/b"]
    );
    assert_eq!(tree.tracked_file_count(), 3);
    assert_eq!(tree.untracked_file_count(), None);

    let root = tree.children("").expect("the repository root");
    assert_eq!(names(&root.files), ["top.md"]);
    assert_eq!(names(&root.folders), ["apps"]);
    let a = tree.children("apps/a").expect("apps/a");
    assert_eq!(names(&a.files), ["README.md"]);
    assert_eq!(names(&a.folders), ["b"]);
    assert_eq!(
        names(&tree.children("apps/a/b").expect("apps/a/b").files),
        ["c.rs"]
    );
}

#[test]
fn files_and_folders_are_told_apart_and_only_tracked_ones_exist() {
    let tree = TrackedTree::from_listing("apps/a/b/c.rs");
    assert!(tree.is_file("apps/a/b/c.rs"));
    assert!(!tree.is_folder("apps/a/b/c.rs"));
    assert!(tree.is_folder("apps/a/b"));
    assert!(!tree.is_file("apps/a/b"));
    assert!(
        tree.is_folder(""),
        "the repository root holds every tracked file"
    );
    assert!(!tree.is_folder("apps/a/c"));
    assert!(tree.children("apps/x").is_none());
}

#[test]
fn siblings_share_their_folders_without_duplicates() {
    let tree = TrackedTree::from_listing("a/b/one.rs\0a/b/two.rs\0a/c/three.rs\0a/four.rs");
    let a = tree.children("a").expect("a");
    assert_eq!(names(&a.files), ["four.rs"]);
    assert_eq!(names(&a.folders), ["b", "c"]);
    assert_eq!(
        names(&tree.children("a/b").expect("a/b").files),
        ["one.rs", "two.rs"]
    );
}

#[test]
fn an_empty_listing_is_an_empty_tree() {
    let tree = TrackedTree::from_listing("");
    assert_eq!(tree.tracked_file_count(), 0);
    assert_eq!(tree.folders().count(), 0);
    assert!(!tree.is_folder(""));
}

#[test]
fn names_with_spaces_survive_the_listing() {
    let tree = TrackedTree::from_listing("apps/With space/README.md\0");
    assert!(tree.is_folder("apps/With space"));
    assert!(tree.is_file("apps/With space/README.md"));
}

#[test]
fn untracked_paths_join_the_tree_like_tracked_ones_and_are_counted_apart() {
    let mut tree = TrackedTree::from_listing("apps/a/main.rs\0");
    tree.add_untracked("apps/a/new.rs\0apps/b/README.md\0apps/a/main.rs\0nested/\0\0");
    assert_eq!(
        tree.files().collect::<Vec<_>>(),
        ["apps/a/main.rs", "apps/a/new.rs", "apps/b/README.md"]
    );
    assert_eq!(
        names(&tree.children("apps").expect("apps").folders),
        ["a", "b"]
    );
    assert!(
        !tree.is_folder("nested") && !tree.is_file("nested"),
        "a nested repository holds no listed file"
    );
    assert_eq!(tree.tracked_file_count(), 1);
    assert_eq!(
        tree.untracked_file_count(),
        Some(2),
        "a path already listed counts once, as tracked"
    );
}

#[test]
fn a_listing_that_exits_non_zero_did_not_run() {
    for (arguments, listing) in [
        (&TRACKED_LISTING[..], "git ls-files -z"),
        (
            &UNTRACKED_LISTING[..],
            "git ls-files --others --exclude-standard -z",
        ),
    ] {
        match listing_text(failed_output(), GIT, arguments) {
            Err(NotRun::ToolError {
                tool,
                status,
                stderr,
            }) => {
                assert_eq!(tool, listing);
                assert_eq!(status, 128);
                assert_eq!(stderr, "fatal: not a git repository");
            }
            other => panic!("a failed listing must not run: {other:?}"),
        }
    }
}

#[test]
fn a_listing_that_exits_zero_is_the_text_it_printed() {
    let output = Output {
        code: 0,
        stdout: "a/b.rs\0".to_string(),
        stderr: String::new(),
        duration: std::time::Duration::ZERO,
    };
    let text = listing_text(output, GIT, &TRACKED_LISTING).expect("a clean listing");
    assert!(TrackedTree::from_listing(&text).is_file("a/b.rs"));
}

#[test]
fn a_missing_listing_program_did_not_run() {
    let root = crate::test_repo_root();
    match run_listing("tbd-no-such-listing-program", &root, &TRACKED_LISTING) {
        Err(NotRun::ToolAbsent(program)) => assert_eq!(program, "tbd-no-such-listing-program"),
        other => panic!("an absent program must not run: {other:?}"),
    }
}

#[test]
fn a_git_that_refuses_its_arguments_did_not_run() {
    let root = crate::test_repo_root();
    match run_listing(GIT, &root, &["ls-files", "--no-such-option"]) {
        Err(NotRun::ToolError { status, .. }) => assert_ne!(status, 0),
        other => panic!("a refused listing must not run: {other:?}"),
    }
}

#[test]
fn untracked_files_join_the_listing_only_when_included_and_ignored_files_never_do() {
    let mut fixture = FixtureCheckout::new("tree-untracked");
    fixture
        .tracked(".gitignore", "*.log\nbuild/\n")
        .tracked("apps/tool/main.rs", "fn main() {}\n")
        .untracked("apps/tool/new.rs", "")
        .untracked("apps/fresh/README.md", "# Fresh\n")
        .untracked("apps/tool/debug.log", "")
        .untracked("apps/tool/build/report.md", "# Report\n");
    let committed = fixture
        .listed_by_git(UntrackedFiles::Invisible)
        .expect("the committed view lists");
    assert_eq!(
        committed.files().collect::<Vec<_>>(),
        [".gitignore", "apps/tool/main.rs"]
    );
    assert_eq!(committed.untracked_file_count(), None);
    let with_untracked = fixture
        .listed_by_git(UntrackedFiles::Included)
        .expect("the untracked view lists");
    assert_eq!(
        with_untracked.files().collect::<Vec<_>>(),
        [
            ".gitignore",
            "apps/fresh/README.md",
            "apps/tool/main.rs",
            "apps/tool/new.rs"
        ]
    );
    assert!(with_untracked.is_folder("apps/fresh"));
    assert!(
        !with_untracked.is_folder("apps/tool/build"),
        "an ignored folder stays invisible"
    );
    assert_eq!(with_untracked.tracked_file_count(), 2);
    assert_eq!(with_untracked.untracked_file_count(), Some(2));
}

#[test]
fn the_checkout_lists_its_committed_layout() {
    let root = crate::test_repo_root();
    let tree = TrackedTree::load(&root, UntrackedFiles::Invisible)
        .expect("git ls-files runs in the checkout");
    assert!(
        tree.tracked_file_count() > 20,
        "too few tracked files: {}",
        tree.tracked_file_count()
    );
    assert_eq!(tree.untracked_file_count(), None);
    assert!(tree.is_file(crate::layout::PROJECT_INSTRUCTIONS));
    assert!(tree.is_folder(crate::layout::DOCUMENTATION_ROOT));
}

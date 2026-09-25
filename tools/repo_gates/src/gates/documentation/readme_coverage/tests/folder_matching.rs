use super::super::entry_pattern::EntryPattern;
use super::*;

const ROOT_LINE: usize = 8;

fn children(files: &[&str], folders: &[&str]) -> FolderChildren {
    FolderChildren {
        files: files.iter().map(ToString::to_string).collect(),
        folders: folders.iter().map(ToString::to_string).collect(),
    }
}

/// Entries on consecutive lines after the root line, from tokens as a Contents block spells them.
fn entries(tokens: &[&str]) -> Vec<ContentsEntry> {
    tokens
        .iter()
        .enumerate()
        .map(|(offset, token)| {
            let (name, kind) = match token.strip_suffix('/') {
                Some(name) => (name, ChildKind::Folder),
                None => (*token, ChildKind::File),
            };
            ContentsEntry {
                line: ROOT_LINE + 1 + offset,
                token: token.to_string(),
                kind,
                pattern: EntryPattern::parse(name).expect("a valid entry"),
            }
        })
        .collect()
}

fn violations(files: &[&str], folders: &[&str], tokens: &[&str]) -> Vec<(usize, String)> {
    match_entries(&children(files, folders), &entries(tokens), ROOT_LINE)
        .into_iter()
        .map(|violation| (violation.line, violation.message))
        .collect()
}

#[test]
fn every_child_matching_exactly_one_entry_is_clean() {
    let found = violations(
        &["README.md", "mod.rs", "scope.rs", "tree.rs"],
        &["tests"],
        &["mod.rs", "{scope,tree}.rs", "tests/"],
    );
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_glob_may_cover_a_whole_collection() {
    let found = violations(&["t1_a.md", "t2_b.md", "t3_c.md"], &[], &["t*_*.md"]);
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn a_child_matched_by_two_entries_is_reported_at_the_first() {
    let found = violations(&["mod.rs"], &[], &["*.rs", "mod.rs"]);
    assert_eq!(
        found,
        [(
            9,
            "tracked child `mod.rs` matches 2 entries (lines 9, 10); it must match exactly one"
                .to_string()
        )]
    );
}

#[test]
fn an_entry_that_matches_no_child_is_reported_at_its_line() {
    let found = violations(&["mod.rs"], &[], &["mod.rs", "gone.rs"]);
    assert_eq!(
        found,
        [(10, "entry `gone.rs` matches no tracked child".to_string())]
    );
}

#[test]
fn a_child_that_matches_no_entry_is_reported_at_the_root_line() {
    let found = violations(&["mod.rs", "extra.rs"], &["tests"], &["mod.rs"]);
    assert_eq!(
        found,
        [
            (
                ROOT_LINE,
                "tracked child `extra.rs` matches no entry".to_string()
            ),
            (
                ROOT_LINE,
                "tracked child `tests/` matches no entry".to_string()
            ),
        ]
    );
}

#[test]
fn the_readme_is_never_a_child() {
    assert!(violations(&["README.md"], &[], &[]).is_empty());
    let listed = violations(&["README.md", "mod.rs"], &[], &["README.md", "mod.rs"]);
    assert_eq!(
        listed,
        [(
            9,
            "entry `README.md` lists the README itself, which Contents leaves out".to_string()
        )]
    );
    let by_glob = violations(&["README.md"], &[], &["*.md"]);
    assert_eq!(
        by_glob,
        [(9, "entry `*.md` matches no tracked child".to_string())]
    );
}

#[test]
fn a_file_entry_never_matches_a_folder_and_says_so() {
    let found = violations(&[], &["tests"], &["tests"]);
    assert_eq!(
        found,
        [
            (ROOT_LINE, "tracked child `tests/` matches no entry".to_string()),
            (
                9,
                "entry `tests` is a file entry, but `tests/` is a folder; a folder entry ends in `/`"
                    .to_string()
            ),
        ]
    );
    let reversed = violations(&["mod.rs"], &[], &["mod.rs/"]);
    assert_eq!(
        reversed[1],
        (
            9,
            "entry `mod.rs/` is a folder entry, but `mod.rs` is a file; only a folder entry ends \
             in `/`"
                .to_string()
        )
    );
}

#[test]
fn untracked_children_never_reach_matching() {
    // The children come from the tracked tree alone; an entry for a file that exists only on
    // disk matches nothing.
    let found = violations(&["mod.rs"], &[], &["mod.rs", "scratch.rs"]);
    assert_eq!(
        found,
        [(
            10,
            "entry `scratch.rs` matches no tracked child".to_string()
        )]
    );
}

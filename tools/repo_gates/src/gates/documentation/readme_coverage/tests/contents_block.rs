use super::*;

const FOLDER: &str = "tools/example";

/// A README whose Contents section holds `block` verbatim, fence lines included. Line 5 is the
/// heading, line 7 the opening fence, line 8 the root line.
fn readme(block: &str) -> String {
    format!("# Example\n\nPurpose.\n\n## Contents\n\n{block}\n\n## Boundaries\n\n- Rules.\n")
}

fn contents(entries: &str) -> String {
    readme(&format!("```text\n{FOLDER}/\n{entries}\n```"))
}

fn read(text: &str) -> ContentsBlock {
    read_contents(text, FOLDER).expect("a Contents block")
}

fn messages(block: &ContentsBlock) -> Vec<(usize, &str)> {
    block
        .violations
        .iter()
        .map(|violation| (violation.line, violation.message.as_str()))
        .collect()
}

fn assert_clean(block: &ContentsBlock) {
    assert!(block.violations.is_empty(), "{:?}", block.violations);
}

fn tokens(block: &ContentsBlock) -> Vec<(&str, ChildKind)> {
    block
        .entries
        .iter()
        .map(|entry| (entry.token.as_str(), entry.kind))
        .collect()
}

fn entry_lines(block: &ContentsBlock) -> Vec<usize> {
    block.entries.iter().map(|entry| entry.line).collect()
}

#[test]
fn tree_prefixes_plain_lines_and_short_indents_are_direct_children() {
    let block = read(&contents(
        "├── mod.rs        the registry\n└── tests/        unit tests\nplain.rs  no prefix\n  \
         indented.rs  two spaces\n    four.rs  four spaces",
    ));
    assert_clean(&block);
    assert_eq!(block.root_line, 8);
    assert_eq!(
        tokens(&block),
        [
            ("mod.rs", ChildKind::File),
            ("tests/", ChildKind::Folder),
            ("plain.rs", ChildKind::File),
            ("indented.rs", ChildKind::File),
            ("four.rs", ChildKind::File),
        ]
    );
    assert_eq!(entry_lines(&block), [9, 10, 11, 12, 13]);
}

#[test]
fn globs_are_entries_of_their_own_kind() {
    let block = read(&contents(
        "├── *.rs          sources\n├── t?_[a-c]{x,y}.md  records\n└── fixture_*/    fixtures",
    ));
    assert_clean(&block);
    assert_eq!(block.entries[0].kind, ChildKind::File);
    assert!(block.entries[0].pattern.matches("mod.rs"));
    assert!(block.entries[1].pattern.matches("t1_bx.md"));
    assert_eq!(block.entries[2].kind, ChildKind::Folder);
    assert!(block.entries[2].pattern.matches("fixture_maps"));
}

#[test]
fn a_nested_line_is_a_violation_and_never_an_entry() {
    let block = read(&contents(
        "├── a/            outer\n│   └── b.rs      nested\n└── c/            last\n    └── d.rs  \
         nested under the last\n      e.rs  six spaces",
    ));
    assert_eq!(
        tokens(&block),
        [("a/", ChildKind::Folder), ("c/", ChildKind::Folder)]
    );
    assert_eq!(
        messages(&block),
        [
            (
                10,
                "entry `b.rs` sits deeper than a direct child; Contents lists direct children only"
            ),
            (
                12,
                "entry `d.rs` sits deeper than a direct child; Contents lists direct children only"
            ),
            (
                13,
                "entry `e.rs` sits deeper than a direct child; Contents lists direct children only"
            ),
        ]
    );
}

#[test]
fn a_slash_inside_an_entry_is_a_violation() {
    let block = read(&contents(
        "├── a/b.rs        nested path\n└── c/d/          nested folder",
    ));
    assert!(block.entries.is_empty());
    assert_eq!(
        messages(&block),
        [
            (
                9,
                "entry `a/b.rs` holds a `/` inside it; Contents lists direct children only"
            ),
            (
                10,
                "entry `c/d/` holds a `/` inside it; Contents lists direct children only"
            ),
        ]
    );
}

#[test]
fn an_entry_needs_two_spaces_before_a_non_empty_role() {
    let block = read(&contents(
        "├── one.rs a single space\n├── two.rs\n└── three.rs    ",
    ));
    assert_eq!(
        tokens(&block),
        [
            ("one.rs", ChildKind::File),
            ("two.rs", ChildKind::File),
            ("three.rs", ChildKind::File),
        ],
        "a missing role still lists the child"
    );
    let reason = "has no role; two or more spaces separate an entry from its role";
    assert_eq!(
        messages(&block),
        [
            (9, format!("entry `one.rs` {reason}").as_str()),
            (10, format!("entry `two.rs` {reason}").as_str()),
            (11, format!("entry `three.rs` {reason}").as_str()),
        ]
    );
}

#[test]
fn spacer_lines_between_entries_are_skipped_and_keep_line_numbers() {
    let block = read(&contents(
        "├── a.rs  first\n│\n├── b.rs  second\n│   │   \n├──\n└── c.rs  last",
    ));
    assert_clean(&block);
    assert_eq!(
        tokens(&block),
        [
            ("a.rs", ChildKind::File),
            ("b.rs", ChildKind::File),
            ("c.rs", ChildKind::File),
        ]
    );
    assert_eq!(entry_lines(&block), [9, 11, 14]);
}

#[test]
fn trailing_spacer_lines_are_skipped() {
    let block = read(&contents("├── a.rs  first\n└── b.rs  last\n│\n    │ \t"));
    assert_clean(&block);
    assert_eq!(entry_lines(&block), [9, 10]);
}

#[test]
fn a_block_of_spacer_lines_alone_lists_nothing() {
    let block = read(&contents("│\n│   │\n└── "));
    assert_clean(&block);
    assert!(block.entries.is_empty());
}

#[test]
fn a_line_with_a_token_after_its_tree_drawing_is_never_a_spacer() {
    let block = read(&contents("├── a.rs  first\n│   b.rs  nested"));
    assert_eq!(entry_lines(&block), [9]);
    assert_eq!(
        messages(&block),
        [(
            10,
            "entry `b.rs` sits deeper than a direct child; Contents lists direct children only"
        )]
    );
}

#[test]
fn blank_lines_are_skipped_and_keep_line_numbers() {
    let block = read(&contents("\n├── a.rs  first\n\n└── b.rs  last"));
    assert_clean(&block);
    assert_eq!(entry_lines(&block), [10, 12]);
}

#[test]
fn a_malformed_glob_or_an_empty_name_is_a_violation() {
    let block = read(&contents(
        "├── [ab.rs  open class\n├── {a,b.rs  open brace\n└── /  nothing",
    ));
    assert!(block.entries.is_empty());
    assert_eq!(
        messages(&block),
        [
            (9, "entry `[ab.rs` has an unclosed `[`"),
            (10, "entry `{a,b.rs` has an unclosed `{`"),
            (11, "entry `/` names nothing"),
        ]
    );
}

#[test]
fn the_root_line_is_exactly_the_folder_path_and_a_slash() {
    let wrong = read(&readme("```text\ntools/example\n├── a.rs  first\n```"));
    assert_eq!(
        messages(&wrong),
        [(
            8,
            "the root line is `tools/example`, not the folder path `tools/example/`"
        )]
    );
    let trailing_space = read(&readme("```text\ntools/example/   \n```"));
    assert_clean(&trailing_space);
    let empty = read(&readme("```text\n```"));
    assert_eq!(
        messages(&empty),
        [(
            7,
            "the Contents block is empty; its first line is the root line `tools/example/`"
        )]
    );
}

#[test]
fn a_readme_without_the_heading_is_a_violation_at_line_one() {
    let violation = read_contents("# Example\n\n```text\ntools/example/\n```\n", FOLDER)
        .expect_err("no heading");
    assert_eq!(
        violation,
        Violation {
            line: 1,
            message: "no `## Contents` heading".to_string()
        }
    );
}

#[test]
fn a_contents_section_without_a_text_block_is_a_violation_at_the_heading() {
    let bare = readme("- mod.rs: the registry");
    let tagged = readme("```rust\ntools/example/\n```");
    let violation = |text: &str| read_contents(text, FOLDER).expect_err("no text block");
    for text in [&bare, &tagged] {
        assert_eq!(
            violation(text),
            Violation {
                line: 5,
                message: "the `## Contents` section holds no ```text code block".to_string()
            }
        );
    }
    let after_next_section = format!(
        "{}\n```text\n{FOLDER}/\n```\n",
        readme("No block in the section.")
    );
    assert_eq!(violation(&after_next_section).line, 5);
}

#[test]
fn another_block_before_the_text_block_is_skipped() {
    let text = readme(&format!(
        "```rust\nfn main() {{}}\n```\n\n```text\n{FOLDER}/\n```"
    ));
    let block = read(&text);
    assert_eq!(block.root_line, 12);
    assert_clean(&block);
}

#[test]
fn headings_and_blocks_inside_other_fences_do_not_count() {
    let text = format!(
        "# Example\n\n````markdown\n## Contents\n\n```text\nnot/the/folder/\n```\n````\n\n\
         ## Contents\n\n```text\n{FOLDER}/\n```\n"
    );
    let block = read(&text);
    assert_eq!(block.root_line, 14);
    assert_clean(&block);
}

#[test]
fn a_block_that_never_closes_is_a_violation_at_its_fence() {
    let text = format!("# Example\n\n## Contents\n\n```text\n{FOLDER}/\n├── a.rs  first\n");
    let violation = read_contents(&text, FOLDER).expect_err("unclosed");
    assert_eq!(
        violation,
        Violation {
            line: 5,
            message: "the Contents block never closes".to_string()
        }
    );
}

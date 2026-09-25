# README coverage parts

The three parts behind the `readme-coverage` gate's Contents rule: reading a README's Contents
block, matching an entry's name or glob, and pairing the entries with the folder's listed
children.

## Contents

```text
tools/repo_gates/src/gates/documentation/readme_coverage/
├── contents_block.rs   finds the Contents block and reads its root line and entry lines
├── entry_pattern.rs    compiles an entry's name or glob into an anchored matcher
├── folder_matching.rs  pairs every listed child with exactly one entry of its own kind
└── tests/              unit tests, one file per part
```

## How it works

The gate in `tools/repo_gates/src/gates/documentation/readme_coverage.rs` calls the three parts in
order for every README it judges:

```text
README text ──> contents_block::read_contents(readme, folder)
                   │  the first `text` block after `## Contents`, before the next `## ` heading
                   │  root line == "<folder>/"; each other line → ContentsEntry or Violation
                   ▼
               entries (token, kind, EntryPattern) ──> folder_matching ──> Violations at README lines
                                                          ▲
                                   the folder's listed files and folders (TrackedTree)
```

`contents_block.rs` skips headings and blocks that sit inside another fenced block, skips blank
lines and spacer lines made only of tree drawing (`│`), and reads every other line as one direct
child: an optional `├── ` or `└── ` prefix (or at most four spaces), the name or glob, two or more
spaces, and a non-empty role. A nested line (`│   ├── `), a `/` inside a name, an empty name or a
malformed glob is a violation at its own line. A README without the heading is a violation at
line 1; a Contents section without a `text` block, or a block that never closes, is a violation at
the heading or the fence.

`entry_pattern.rs` makes a name without glob characters match only itself, and compiles `*`, `?`,
`[…]` and `{a,b}` into an anchored regular expression that matches whole names, a leading dot
included. Other regular-expression characters stay literal.

`folder_matching.rs` checks that every listed direct child except README.md matches exactly one
entry of its own kind (a folder entry ends in `/`) and that every entry matches at least one
child. A child no entry matches is reported at the root line; every other violation at the line
of the entry it concerns.

## Boundaries

- Depends on: `tools/repo_gates/src/gate_run/markdown_fences.rs`; the parent gate's `ChildKind`
  and `Violation`; `regex` for globs.
- Used by: `tools/repo_gates/src/gates/documentation/readme_coverage.rs` only.
- Rules:
  - a nested line is a violation and never an entry (`a_nested_line_is_a_violation_and_never_an_entry`);
  - headings and blocks inside other fences do not count
    (`headings_and_blocks_inside_other_fences_do_not_count`);
  - a file entry never matches a folder, and README.md is never a child
    (`a_file_entry_never_matches_a_folder_and_says_so`, `the_readme_is_never_a_child`);
  - a malformed glob is refused, never compiled into a looser pattern
    (`a_malformed_glob_is_refused_with_its_reason`, `regular_expression_characters_are_literal`).

## Related documentation

- [The Contents block](/documentation/standards/readme_standard.md#the-contents-block) — the
  grammar these parts read.

# Source gates

The gates that judge source and text files one at a time: which languages the repository holds,
how long a Rust file may grow, how a module opens, where its tests live, how comments and
documents word things, and the whitespace rules of `.editorconfig`.

## Contents

```text
tools/repo_gates/src/gates/source/
├── editorconfig.rs    `editorconfig`: UTF-8, LF line ends, a final newline, no trailing whitespace
├── file_length.rs     `file-length`: production Rust under 500 lines, test files under 1000
├── language_bans.rs   `language-bans`: an allow list of file kinds, and no script shebangs
├── mod.rs             the module tree of the six gates
├── module_headers.rs  `module-headers`: the Role, Position, Signals and state, Invariants header
├── prose_rules.rs     `prose-rules`: no ticket ids, no history words, no milestone ids in code
├── test_placement.rs  `test-placement`: unit tests in sibling files under `tests/`, never inline
└── tests/             unit tests, one file per gate
```

## How it works

Each gate is a unit struct implementing `gate_run::file_rule::FileRule`: `selects` picks the
tracked files it judges, and `problems` returns one line per problem found in a file's bytes.
`gates::run` hands it to `verify_file_rule`, which lists, scopes, reads and reports.

- `language-bans` judges every listed file. It passes Rust, Markdown, TOML and JSON (but not a
  Node manifest), `Cargo.lock`, `.gitignore`, `.gitattributes`, `.editorconfig`, and `srt`, `vtt`
  or `ass` subtitle fixtures inside a `tests/fixtures/` folder. The first line must not be a
  shebang naming a shell, Python, Node, Deno or Bun; the shebang is parsed, so Rust's `#![…]` is
  not one.
- `file-length` judges every `.rs` file; one inside a `tests` folder is a test file.
- `module-headers` judges every production `lib.rs` and `main.rs` and every production `.rs` file
  of 80 lines or more: its leading `//!` block names **Role:**, **Position:**, **Signals and
  state:** and **Invariants:**, in that order.
- `test-placement` parses every production `.rs` file with `syn`: an inline `cfg(test)` module or
  a test function fails, and an out-of-line test module must carry
  `#[path = "tests/<file>.rs"]`. Strings and comments never count; a file that does not parse
  fails with the parser's message.
- `prose-rules` judges production Rust files, every README.md, live documents under the
  documentation root and `CLAUDE.md`. A ticket id fails anywhere; a history word (whole word, any
  case) fails outside the decision log in `documentation/decisions/`; a milestone id fails in
  Rust files and code READMEs. The history words are split in two halves in the source so the
  file never flags itself.
- `editorconfig` judges every listed file byte for byte; Markdown may end a line in whitespace,
  since two trailing spaces are a hard break there. Each problem names the first line that breaks
  its rule.

## Boundaries

- Depends on: `tools/repo_gates/src/gate_run/` (`FileRule`, `is_test_path`, `extension`,
  `path_regions`); `tools/repo_gates/src/layout.rs`; `regex` (prose rules); `syn` and
  `proc-macro2` (test placement).
- Used by: `tools/repo_gates/src/gates/mod.rs`, for the six gates.
- Rules:
  - the language rule is an allow list with no exemption list, so a new script kind is banned
    before anyone names it (`scripts_build_files_and_stray_kinds_are_banned`,
    `the_shebang_is_parsed_not_matched`);
  - the file-length limits have no exemption list (`production_files_stop_below_500_lines`,
    `test_files_stop_below_1000_lines`);
  - only the leading `//!` block counts as a header (`a_label_after_the_leading_block_does_not_count`);
  - text inside strings and comments is never a test (`strings_and_comments_never_count`);
  - frozen records and test files are outside the prose rules, and the decision log may use
    history words (`frozen_records_and_tests_are_not_judged`,
    `milestones_are_allowed_in_documents_and_history_in_the_decision_log`).

## Related documentation

- [Coding standards](/documentation/standards/coding_standards.md#files-and-tests) — the file
  length and test placement laws.
- [Rust only](/documentation/decisions/foundations.md#2026-09-25--rust-only-ffmpeg-as-the-one-external-program)
  — why the language ban exists.

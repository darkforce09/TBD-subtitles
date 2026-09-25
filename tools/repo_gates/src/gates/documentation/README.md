# Documentation gates

The gates that hold the documentation rules: every folder in the README span carries a README whose
Contents block matches it and whose sections follow the standard, Markdown lives where it belongs,
documents open with a status line, and every link, backticked path and cited `cargo gates` command
exists.

## Contents

```text
tools/repo_gates/src/gates/documentation/
├── link_check/            the link-check scan and rules: Markdown parsing, links, paths, commands
├── link_check.rs          the `link-check` gate: the rule pipeline, break listing and totals
├── markdown_placement.rs  the `markdown-placement` gate: README-only code trees, 500-line documents
├── mod.rs                 the module tree of the five gates
├── readme_coverage/       Contents block parsing, entry globs and folder matching
├── readme_coverage.rs     the `readme-coverage` gate: a README in every span folder, Contents matching
├── readme_sections.rs     the `readme-sections` gate: title, `##` headings in order, Boundaries bullets
├── status_lines.rs        the `status-lines` gate: the first line of every document under the root
└── tests/                 whole-gate tests over fixture checkouts, one file per gate
```

## How it works

Every gate here judges the files `git ls-files` lists (and, under `--with-untracked`, the untracked
files git does not ignore), narrowed by `--path`, and takes its regions from
`tools/repo_gates/src/gate_run/path_regions.rs`.

- **`readme-coverage`** walks every listed folder of the README span (the code trees and the
  documentation root, minus folders named `tests` or `generated` or starting with `.`) and judges
  two rules per folder: the folder carries a listed README.md, and that README's Contents block
  lists exactly the folder's listed direct children. `readme_coverage/` reads the block and
  matches it; each violation prints as `path:line: message`.
- **`readme-sections`** is a file rule over every README.md in the span: the first heading is the
  H1 title, a code README carries no status line, every `##` heading is one the README standard
  names, in its order and at most once, `## Contents` and `## Boundaries` are present, and
  Boundaries holds exactly the `Depends on:`, `Used by:` and `Rules:` bullets. Which kind sections
  a folder needs stays with the writer.
- **`markdown-placement`** judges two rules: a code tree holds no Markdown but README.md (exempt
  folders aside), and every live document under the documentation root is at most 500 lines.
- **`status-lines`** is a file rule over every Markdown file under the documentation root: the
  first line is `**Status:** live`, `**Status:** frozen record (YYYY-MM-DD)` or
  `**Status:** archived`, and a document other than a README.md in a frozen folder is never live.
- **`link-check`** scans each judged document once as a renderer would and runs three rules over
  the scan: links, backticked repository paths and cited `cargo gates` commands.
  `link_check/` holds the scan and the rules.

### Which gate judges what

| Area | readme-coverage, readme-sections | markdown-placement | status-lines | link-check |
|---|---|---|---|---|
| a folder named `tests` or `generated`, or starting with `.`, and all below it | no README needed, none checked | in a code tree, may hold any Markdown; under the documentation root, still size-limited | judged under the documentation root | judged like any other document |
| frozen records (the frozen folders in `tools/repo_gates/src/layout.rs`, their README.md aside) | judged, when a folder | outside the size limit | frozen record or archived, never live | links only |
| the project instructions (`CLAUDE.md`) | outside the span | outside every rule | not judged | judged as live |
| a README.md outside the span, the repository root's included | not judged | not judged | not judged | judged as live |

## Public surface

- `readme_coverage::verify_readme_coverage`, `markdown_placement::verify_markdown_placement` and
  `link_check::verify_link_check` (with `BreakListing`), called by `gates::run`.
- `readme_sections::ReadmeSections` and `status_lines::StatusLines`, the file rules
  `gates::run` hands to `verify_file_rule`.

## Boundaries

- Depends on: `tools/repo_gates/src/gate_run/` (listing, scope, regions, fences, run report);
  `tools/repo_gates/src/layout.rs`; `tools/repo_gates/src/cli.rs` (the command tree the link check
  walks); `verification_core`; `regex` (Contents globs, status lines); the `git` program
  (`ls-files`, and `check-ignore` in the link check).
- Used by: `tools/repo_gates/src/gates/mod.rs`, for the five gates.
- Rules:
  - a check that could not examine its input reports "did not run", never a pass
    (`a_failed_or_empty_listing_did_not_run`, `a_refused_or_empty_scope_did_not_run`,
    `a_tracked_readme_missing_from_the_disk_did_not_run`,
    `an_unreadable_document_or_a_failed_listing_did_not_run`,
    `a_document_missing_from_the_disk_did_not_run`);
  - untracked files count only under `--with-untracked`, and ignored ones never do
    (`an_untracked_readme_makes_its_folder_pass_only_with_untracked_files_included`,
    `untracked_markdown_is_placed_only_with_untracked_files_included`,
    `a_link_to_an_untracked_file_resolves_only_with_untracked_files_included`);
  - the exemptions above hold (`test_generated_and_hidden_folders_need_no_readme`,
    `a_readme_inside_an_exempt_folder_is_not_held_to_its_contents`,
    `frozen_records_are_outside_the_limit`, `a_frozen_folder_holds_no_live_document_but_its_index`,
    `every_judged_area_is_judged_and_nothing_else`);
  - a code README carries no status line and Boundaries holds the three bullets in order
    (`a_code_readme_carries_no_status_line`, `boundaries_holds_the_three_bullets_in_order`).

## Related documentation

- [README standard](/documentation/standards/readme_standard.md) — the rules `readme-coverage` and
  `readme-sections` hold, and the Contents block grammar.
- [Documentation standards](/documentation/standards/documentation_standards.md) — status lines,
  where documents live, and their size.

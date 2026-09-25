# Gate machinery

What every gate shares: the listing of the files it judges, the `--path` scope, the questions a
gate asks about where a path sits, fenced-block recognition in Markdown, and the run that carries a
gate's verdicts to the shared report and its exit status.

## Contents

```text
tools/repo_gates/src/gate_run/
├── file_rule.rs        the `FileRule` trait and the loop that judges one tracked file at a time
├── gate_scope.rs       resolves and refuses `--path` values, and answers whether a path is in scope
├── markdown_fences.rs  the lines that open and close a fenced code block, per CommonMark
├── mod.rs              `GateRequest`, `GateRun`, `Tally`, `prepare` and `read_tracked`
├── path_regions.rs     where a path sits: code tree, documentation root, README span, exempt folder
├── tests/              unit tests, plus a temporary git checkout helper for whole-gate tests
└── tracked_tree.rs     the listed files and the folders they imply, from `git ls-files`
```

## How it works

A gate run has three steps, and every gate takes them the same way:

```text
TrackedTree::load ── git ls-files -z  (+ --others --exclude-standard under --with-untracked)
        │
prepare(label, listing, request) ── GateScope::resolve(--path values, tree)
        │                              a failed or empty listing, or a refused scope → a stopped run
        ▼
the gate judges what the scope selects ── one Verdict per item ──> GateRun ──> print → 0 / 1 / 2
```

`tracked_tree.rs` runs `git ls-files` through `verification_core::proc::Run` with a 120-second
deadline and turns the NUL-separated listing into a set of files plus, for every folder that holds
one, its direct children. A folder exists exactly when a listed file sits at or below it. Git
missing, killed, timed out or exiting non-zero is a `NotRun` cause, never an empty tree.

`gate_scope.rs` turns each `--path` value into a repository-relative folder: no value, `.` or the
checkout root means the whole repository, and an absolute value inside the checkout is made
relative. A value that climbs out with `..`, lies outside the checkout, names a file or names no
tracked folder is refused, so a typo never narrows a gate to nothing and reads as a pass.

`mod.rs` holds `GateRequest` (the scope values and whether untracked files count), `prepare`
(listing plus scope, or the stopped run that says why nothing was judged), `judged_nothing` (the
"did not run" verdict for an empty selection) and `GateRun`, which prints the header, every verdict
through `verification_core::Report`, the totals and the summary line. A run that included
untracked files says so on its summary line, so its result never passes for a check of the
committed files.

`file_rule.rs` is the loop behind every gate that judges files one at a time: the rule says which
tracked files it selects and returns its problems for each file's bytes; a file with problems is
one `Failed` verdict listing them, and a file the disk lacks is "did not run".
`path_regions.rs` answers the region questions from `crate::layout`: a folder named `tests` or
`generated`, or starting with `.`, is exempt from the README rules with everything below it.
`markdown_fences.rs` lets the README and link gates skip headings and blocks that sit inside
another fenced block.

## Boundaries

- Depends on: `verification_core` (`Verdict`, `NotRun`, `Report`, `proc::Run`); `crate::layout`;
  the `git` program.
- Used by: every gate under `tools/repo_gates/src/gates/`, and `tools/repo_gates/src/cli.rs` for
  `GateRequest` and `UntrackedFiles`.
- Rules:
  - a failed or empty listing, an unreadable file and a scope that selects nothing are "did not
    run", never a pass (`a_listing_that_exits_non_zero_did_not_run`,
    `a_missing_listing_program_did_not_run`, `an_unreadable_file_or_nothing_selected_did_not_run`);
  - untracked files count only under `--with-untracked`, and ignored files never do
    (`untracked_files_join_the_listing_only_when_included_and_ignored_files_never_do`);
  - a `--path` value that names no tracked folder is refused wherever it stands
    (`a_value_that_names_no_tracked_folder_is_refused`,
    `a_bad_value_is_refused_even_beside_a_whole_repository_value`);
  - a path is inside a folder only at a component boundary, and exempt folders take their whole
    subtree with them (`a_path_is_within_a_folder_only_at_a_component_boundary`,
    `test_generated_and_hidden_folders_are_exempt_with_their_subtrees`).

## Related documentation

- [README standard](/documentation/standards/readme_standard.md#which-folders-carry-a-readme) —
  the README span and the exempt folders `path_regions.rs` encodes.

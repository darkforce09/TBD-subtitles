# Verification core source

The modules of the `verification_core` library: the verdict type, the assertions and scans written
against it, the child-process adapter, and the report that turns many verdicts into one exit
status.

## Contents

```text
tools/verification_core/src/
├── gate.rs     `ban` and `require` over files, their `_str` forms over text, and the `probe` pair
├── lib.rs      the crate root: module tree, re-exports and the invariants every module keeps
├── pattern.rs  `Pattern`, a compiled regex or escaped literal whose `^` and `$` anchor lines
├── proc.rs     maps each way a child run stopped onto a `NotRun` cause; `RunVerdict` and `which`
├── report.rs   `Report`: accumulates verdicts, prints each failure, yields exit 0, 1 or 2
├── scan.rs     a sorted, fail-closed tree walk and the matching lines with their paths
├── tests/      unit tests, one file per module
└── verdict.rs  `Verdict`, `NotRun`, `Finding` and `Kind`, and how a failure renders
```

## How it works

`verdict.rs` is the vocabulary every other module speaks. `Verdict` has three variants; its
`DidNotRun` keeps the structured `NotRun` cause beside the rendered `Finding`, so a test can assert
on the reason. A `Finding` renders as one headline plus six-space-indented continuation lines, the
same in every gate.

The checks come in three shapes:

```text
gate.rs    ban / require        files in, one Verdict out     (a file that cannot be read → DidNotRun)
           ban_str / require_str text in, one Verdict out
           probe_files / probe_str  Result<bool, NotRun>, so `?` carries "did not run" upward
scan.rs    walk_files           roots in, sorted file list out (a missing root → DidNotRun, never zero hits)
           matching_lines       every hit as path:line:text
proc.rs    RunVerdict           a child run in, one Verdict out (expect_ok, expect_code)
```

`pattern.rs` compiles every pattern with line anchors turned on unconditionally, so `^foo` means
"some line begins with foo" as it does for a line-oriented search tool; a literal needle is escaped
rather than trusted. `gate.rs` joins file contents with a newline boundary so a match never spans
two files. `scan.rs` never follows a symbolic link and sorts its output, so two runs over one tree
print the same log.

`proc.rs` re-exports `Run`, `Output`, `Merged` and `RunError` from `crates/child_process`, which
spawns each child in its own process group, drains both pipes and kills the group on a timeout. It
converts every `RunError` into a `NotRun`: an absent program is `ToolAbsent`, a signal
`Signalled`, a deadline `Timeout`, and a spawn or wait failure `ToolError`. Exit codes pass
through raw.

`report.rs` counts the checks run, failed and not run, prints each failure as it lands, and
`finish` prints one summary line and yields 0, 1 or 2, a check that did not run outranking a
violation. `Verdict::into_exit` gives the same three codes for a single verdict.

## Public surface

- `verdict`: `Verdict`, `NotRun`, `Finding`, `Kind` (also re-exported at the root).
- `report`: `Report` (also at the root), with `check`, `clean`, `finish` and `counts`.
- `pattern`: `Pattern` (also at the root), with `regex`, `literal` and `case_insensitive`.
- `gate`: `ban`, `require`, `ban_str`, `require_str`, `probe_files`, `probe_str`.
- `scan`: `walk_files`, `matching_lines`, `with_extension` and the `Hit` they return.
- `proc`: `Run`, `Output`, `Merged`, `RunError`, the `RunVerdict` trait and `which`.

## Boundaries

- Depends on: `crates/child_process` (in `proc.rs` only), `regex` (in `pattern.rs` only) and `std`.
- Used by: `tools/repo_gates`, which uses `verdict`, `report` and `proc`.
- Rules:
  - `Verdict` never converts to or from `bool` (`exit_codes_separate_did_not_run_from_failed`
    in `tests/verdict_tests.rs` and the exhaustive `match` in every caller);
  - a target, root or program that could not be examined is `DidNotRun`, never a pass
    (`missing_target_is_did_not_run_not_held`, `a_missing_root_is_did_not_run_not_zero_hits`,
    `probe_propagates_did_not_run_instead_of_short_circuiting_clean`,
    `expect_ok_maps_an_absent_program_to_did_not_run`);
  - a signal or a timeout is `DidNotRun`, never `Failed` (`signal_death_is_did_not_run_never_failed`,
    `a_signal_and_a_timeout_stay_did_not_run`);
  - "did not run" outranks a violation in the exit status (`a_did_not_run_outranks_violations`);
  - `^` and `$` anchor lines and a match never spans two files (`caret_is_a_line_anchor`,
    `patterns_cannot_match_across_a_file_boundary`).

## Related documentation

- [Verification core](/tools/verification_core/README.md) — the crate, its exit contract and its
  dependencies.

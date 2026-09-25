# Verification core

The `verification_core` library: the small, fail-closed check library every repository gate is
written with. A check has three outcomes instead of two (held, failed, did not run), so a check
whose input is missing, whose program is absent or whose child process was killed never reports a
pass.

## Contents

```text
tools/verification_core/
├── Cargo.toml  the `verification_core` library package; it depends on `crates/child_process` and `regex`
└── src/        verdicts, pattern assertions, tree scans, the child-process adapter and the report
```

## How it works

A check produces a `Verdict`: `Held`, `Failed` with a rendered `Finding`, or `DidNotRun` with the
`NotRun` cause (a missing or unreadable target, an absent or failing program, a signal, a
timeout). `Verdict` has no `From<bool>` and no `is_ok()`, so no expression folds "did not run"
into "passed" by accident, and adding a `NotRun` variant breaks every `match` that does not handle
it. A gate passes its verdicts to a `Report`, whose `finish` prints the summary line and returns
the exit status:

| Exit | Meaning |
|---|---|
| 0 | every check ran and held |
| 1 | at least one check failed, and every check ran |
| 2 | at least one check did not run; this outranks 1, since a check that did not run is not a pass |

The dependencies are few on purpose, since every gate build pays for them. `regex` compiles the
pattern matcher into the gate, so no check depends on a search binary being installed.
`crates/child_process` spawns, times out and drains every child (`git`, `cargo`), killing a
timed-out child's whole process group; the `proc` module here only maps each way a run stopped
without an exit code onto a `NotRun` cause. [The source README](/tools/verification_core/src/README.md)
describes each module.

## Getting started

Run these from the repository root:

```bash
cargo test -p verification_core                 # the unit tests; they run sh and sleep as child processes
cargo gates readme-coverage --path tools/verification_core   # any gate exercises the crate
```

## Configuration

None: the crate reads no setting of its own. Resolving a program reads `PATH`, inside
`crates/child_process`.

## Public surface

- The library `verification_core`, with `Verdict`, `NotRun`, `Finding`, `Kind`, `Pattern` and
  `Report` at its root and the modules `gate`, `pattern`, `proc`, `report`, `scan` and `verdict`.
  [The source README](/tools/verification_core/src/README.md) says what each module offers.
- No binary.

## Boundaries

- Depends on: `crates/child_process` (child processes) and `regex` 1; no other workspace crate.
- Used by: `tools/repo_gates`, by path dependency: its gates speak `Verdict` and `NotRun`, print
  through `Report`, and list files with `proc::Run`.
- Rules:
  - the crate depends on no workspace crate but `child_process` (`cargo gates crate-layering`,
    which reads the tool table in `tools/repo_gates/src/layout.rs`);
  - "did not run" never becomes a pass, which each module's tests in `src/tests/` hold
    (`a_did_not_run_outranks_violations`, `missing_target_is_did_not_run_not_held`,
    `a_missing_root_is_did_not_run_not_zero_hits`, `a_signal_and_a_timeout_stay_did_not_run`).

## Related documentation

- [Gate runner](/tools/repo_gates/README.md) — the gates built on this crate.
- [Coding standards](/documentation/standards/coding_standards.md#errors-and-processes) — how
  errors and child processes are handled across the repository.

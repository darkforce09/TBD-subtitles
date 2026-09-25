# Gate runner

The `repo_gates` binary behind `cargo gates`: it checks every repository law a program can check
over the tracked files and exits 0 when all held, 1 on a violation, and 2 when a check could not
run.

## Contents

```text
tools/repo_gates/
├── Cargo.toml  the `repo_gates` binary package and the reason for each dependency
└── src/        the command line, the shared gate machinery and one module per gate
```

## How it works

`cargo gates` is an alias in `.cargo/config.toml` for `cargo run --quiet --package repo_gates --`.
The binary finds the workspace root (the nearest folder at or above the working directory whose
`Cargo.toml` declares `[workspace]`), lists the tracked files with `git ls-files`, and runs the gate
named on the command line, or every gate in order when none is named. Each gate prints a header, one
line per failure, its totals and a summary line; running every gate exits with the worst status.

```text
cargo gates [<gate>] ──> cli ──> gates::run ──> one gate ──> gate_run (listing, scope, report)
                                                    │
                                                    └── verification_core::Report → exit 0 / 1 / 2
```

Every gate judges the files `git ls-files` lists, never a walk of the disk, so build output and
ignored files are never judged. `--with-untracked` adds the untracked files git does not ignore,
so new files can be checked before they are committed; such a run says so on its summary line.
A gate that could not list the files, could not read one it judges, or whose scope selects nothing
reports "did not run", never a pass. [The source README](/tools/repo_gates/src/README.md) lists
each gate and the law it holds.

## Getting started

Run these from the repository root:

```bash
cargo gates                                   # every gate over the committed files
cargo gates file-length                       # one gate
cargo gates readme-sections --path tools --with-untracked   # one folder, new files included
cargo gates link-check --report               # every link break in full
cargo test -p repo_gates                      # the unit tests; they need git on the PATH
```

A clean run ends each gate with a line such as `file-length: OK — 40 check(s), all held`.

## Configuration

None: the binary reads no environment variable and no settings file of its own. Where things live
is fixed in `src/layout.rs`: the code trees, the documentation root, the frozen folders, the
project instructions, and the layer table of the product crates and tools. The alias lives in
`.cargo/config.toml`. `git` is found on `PATH`.

## Public surface

- The binary `repo_gates`, run as `cargo gates [<gate>] [--report] [--path <dir>]... [--with-untracked]`;
  [the source README](/tools/repo_gates/src/README.md#commands) describes each gate and flag.
- No library: every module is private to the binary.

## Boundaries

- Depends on: `tools/verification_core` (verdicts, the report, child processes); `clap` for the
  command line and the command tree the link check walks; `regex`, `syn` with `proc-macro2`, and
  `toml`; the `git` program (`ls-files`, `check-ignore`, and `init`/`add` in the tests).
- Used by: people and agents from the repository root, through the `gates` alias in
  `.cargo/config.toml`; no crate depends on it.
- Rules:
  - it depends on no workspace crate but `verification_core` (`cargo gates crate-layering` over
    the tool table in `src/layout.rs`, and `the_workspace_holds_its_layers`);
  - every gate takes `--path` and `--with-untracked`
    (`every_gate_takes_a_repeatable_path_and_the_untracked_flag`);
  - a gate that could not examine its input exits 2, never 0
    (`a_failed_or_empty_listing_did_not_run`, `an_unreadable_file_or_nothing_selected_did_not_run`).

## Related documentation

- [README standard](/documentation/standards/readme_standard.md) — the README rules the
  documentation gates hold.
- [Coding standards](/documentation/standards/coding_standards.md) — the source rules the source
  and workspace gates hold.
- [Documentation standards](/documentation/standards/documentation_standards.md) — status lines,
  placement and size of documents.
- [Repository tooling may run git and cargo](/documentation/decisions.md#2026-09-25--repository-tooling-may-run-git-and-cargo)
  — why the gates may list files with `git`.

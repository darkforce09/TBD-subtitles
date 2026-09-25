# Repository tools

The Rust programs that look after this repository rather than the subtitles: the gate runner that
checks the project laws a program can check, and the fail-closed check library it is built on.
Nothing here ships with the app.

## Contents

```text
tools/
├── repo_gates/         the `cargo gates` runner: one gate per checkable repository law
└── verification_core/  the fail-closed verdict library every gate reports through
```

## How it works

`repo_gates` is a binary crate. `cargo gates` (an alias in `.cargo/config.toml`) runs it from the
repository root; it lists the tracked files with `git ls-files`, judges them gate by gate, and
exits 0 when every check held, 1 when a check found a violation, and 2 when a check could not run.

`verification_core` is a library crate. It gives a check three outcomes instead of two (held,
failed, did not run), so a gate whose input is missing, or whose `git` child process is absent,
killed or timed out, can never report a pass. It starts child processes through
`crates/child_process`, the same runner the app uses.

```text
cargo gates [<gate>] ──> repo_gates ──> verification_core ──> crates/child_process ──> git
                             │                  │
                             │                  └── Verdict, Report (exit 0 / 1 / 2)
                             └── reads the tracked files and judges them
```

Both crates are members of the one Cargo workspace. They may run `git` and `cargo` as child
processes; the app never runs either.

## Getting started

Run these from the repository root:

```bash
cargo gates                                     # every gate over the committed files
cargo gates readme-coverage --path tools        # one gate over one folder
cargo gates link-check --with-untracked         # also judge new files git does not ignore
cargo test -p repo_gates -p verification_core   # the unit tests of both crates
```

A clean gate ends with a summary line such as `readme-coverage: OK — 12 check(s), all held`. The
tests need `git` on the `PATH`: some build a temporary git checkout, and some judge this one.

## Boundaries

- Depends on: `crates/child_process` (through `verification_core`); the crates.io crates `clap`,
  `regex`, `syn`, `proc-macro2` and `toml`; the `git` program.
- Used by: people and agents before a commit, through `cargo gates`; no product crate depends on
  anything here.
- Rules:
  - a tool depends only on the workspace crates the tool table in
    `tools/repo_gates/src/layout.rs` lists for it (`cargo gates crate-layering`, and the
    `the_workspace_holds_its_layers` test);
  - every source file is Rust, with no script of any kind (`cargo gates language-bans`).

## Related documentation

- [Repository tooling may run git and cargo](/documentation/decisions.md#2026-09-25--repository-tooling-may-run-git-and-cargo)
  — why the tools, and only the tools, run `git`.
- [Simple documentation system](/documentation/decisions.md#2026-09-25--simple-documentation-system-no-ticket-machinery)
  — why the checkable laws live in one small gate tool.
- [Coding standards](/documentation/standards/coding_standards.md) — the laws several gates hold.
- [README standard](/documentation/standards/readme_standard.md) — the README rules the
  documentation gates hold.

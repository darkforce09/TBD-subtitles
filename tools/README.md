# Repository tools

The Rust programs that look after this repository rather than the subtitles: the gate runner that
checks the project laws a program can check, the fail-closed check library it is built on, and
the stack spike harness that measures the ML stack on a real video, with its ggml and language-model
workers. Nothing here ships with the app.

## Contents

```text
tools/
├── repo_gates/         the `cargo gates` runner: one gate per checkable repository law
├── stack_spike/        the measuring harness: each ML stack piece on one video, and the model downloads
├── stack_spike_ggml/   the stack spike's worker for the ggml models, a binary of its own
├── stack_spike_llm/    the stack spike's worker for the local language model, a binary of its own
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

`stack_spike` is a binary crate for measuring, not checking: it downloads the pinned models and
the CUDA 13 runtime through `crates/inference`, and runs the stack under test on one video.

`stack_spike_ggml` and `stack_spike_llm` are the spike's workers for the ggml models and the local
language model, each a binary of its own because ggml, candle and ONNX Runtime cannot share a
process.

All five crates are members of the one Cargo workspace. The gate crates may run `git` and
`cargo` as child processes; the app never runs either.

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

- Depends on: `crates/child_process` (through `verification_core`); `crates/inference`,
  `crates/media_io`, `crates/stages` and `crates/job_model` in the stack spike tools, and
  `crates/pipeline` (its measurements) in `stack_spike`; the crates.io crates `clap`, `anyhow`,
  `regex`, `syn`, `proc-macro2` and `toml`; the `git` program.
- Used by: people and agents before a commit, through `cargo gates`, and a developer measuring the
  stack, through `stack-spike`; no product crate depends on anything here.
- Rules:
  - a tool depends only on the workspace crates the tool table in
    `tools/repo_gates/src/layout.rs` lists for it (`cargo gates crate-layering`, and the
    `the_workspace_holds_its_layers` test);
  - every source file is Rust, with no script of any kind (`cargo gates language-bans`).

## Related documentation

- [Repository tooling may run git and cargo](/documentation/decisions/foundations.md#2026-09-25--repository-tooling-may-run-git-and-cargo)
  — why the tools, and only the tools, run `git`.
- [Simple documentation system](/documentation/decisions/foundations.md#2026-09-25--simple-documentation-system-no-ticket-machinery)
  — why the checkable laws live in one small gate tool.
- [Coding standards](/documentation/standards/coding_standards.md) — the laws several gates hold.
- [README standard](/documentation/standards/readme_standard.md) — the README rules the
  documentation gates hold.

# Repository tools

The Rust programs that look after this repository rather than the subtitles: the gate runner that
checks the project laws a program can check, the fail-closed check library it is built on, the
AppImage builder that packs the app for a desktop, the stack spike harness that measures the
ML stack on a real video, with its ggml and language-model workers, the visual validation tool
that checks on-screen text against annotated pilots, and the probe that tests redb across
processes. Nothing here ships with the app; the AppImage builder only packs it.

## Contents

```text
tools/
├── appimage_builder/    the `cargo appimage` builder: the app, its GPU libraries and FFmpeg in one AppImage
├── redb_process_probe/  how redb behaves when a second process opens a job database
├── repo_gates/          the `cargo gates` runner: one gate per checkable repository law
├── stack_spike/         the measuring harness: each ML stack piece on one video, and the model downloads
├── stack_spike_ggml/    the stack spike's worker for the ggml models, a binary of its own
├── stack_spike_llm/     the stack spike's worker for the local language model, a binary of its own
├── verification_core/   the fail-closed verdict library every gate reports through
└── visual_validation/   visual pilot recognition and annotated acceptance checks
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

`appimage_builder` is a binary crate. `cargo appimage` (an alias in `.cargo/config.toml`) builds
the app's three release binaries (the app and its ggml and local language-model workers),
gathers the CUDA, cuDNN and ONNX Runtime libraries through `crates/inference` and a pinned
static FFmpeg, and packs them behind the pinned AppImage runtime
as `dist/TBD-subtitles-x86_64.AppImage`; the ELF fix-ups and the squashfs image are Rust, with no
packaging program.

`stack_spike_ggml` and `stack_spike_llm` are the spike's workers for the ggml models and the local
language model, each a binary of its own because ggml, candle and ONNX Runtime cannot share a
process.

`redb_process_probe` is a binary crate that tests redb rather than the repository: it opens one
database file from two processes (it starts itself again as the second), read-write and
read-only, idle, writing and killed, and prints what redb did as a Markdown table; it also reads
an rkyv archive in place from a redb value and measures the RAM one large write transaction
holds. It depends on no workspace crate.

`visual_validation` is a binary crate that runs the production on-screen text steps on pilot
clips and stills, through `crates/pipeline`, `crates/stages` and `crates/inference`, and checks a
job's stored documents against hand annotations in `visual_validation/pilots/`; its `evaluate`,
`inspect` and probe commands read a job's documents and per-frame rows from its job database
(`job.redb`), the same store the app writes.

All nine crates are members of the one Cargo workspace. The gate crates may run `git` and
`cargo` as child processes; the AppImage builder also runs the app it built and the FFmpeg it
bundles, to check them; the app never runs `git` or `cargo`.

## Getting started

Run these from the repository root:

```bash
cargo gates                                     # every gate over the committed files
cargo gates readme-coverage --path tools        # one gate over one folder
cargo gates link-check --with-untracked         # also judge new files git does not ignore
cargo test -p repo_gates -p verification_core   # the unit tests of both crates
cargo appimage                                  # build and pack dist/TBD-subtitles-x86_64.AppImage
```

A clean gate ends with a summary line such as `readme-coverage: OK — 12 check(s), all held`. The
tests need `git` on the `PATH`: some build a temporary git checkout, and some judge this one.

## Boundaries

- Depends on: `crates/child_process` (through `verification_core`, and directly in
  `stack_spike`, `appimage_builder` and `visual_validation`); `crates/inference`,
  `crates/media_io`, `crates/stages` and `crates/job_model` in the stack spike tools, and
  `crates/pipeline` (its measurements) in `stack_spike`; `crates/inference` and
  `crates/app_icon` in `appimage_builder`; `crates/inference`, `crates/job_model`,
  `crates/stages`, `crates/media_io`, `crates/pipeline`, `crates/worker_channel` and
  `crates/subtitle_formats` in `visual_validation`; the crates.io crates `clap`, `anyhow`,
  `regex`, `syn`, `proc-macro2`, `toml`, `object`, `backhand`, `png`, `image`, `ureq`, `serde`,
  `serde_json`, `redb` and `rkyv`; the `git` and `cargo` programs, and FFmpeg in the tools that
  read video.
- Used by: people and agents before a commit, through `cargo gates`; a developer measuring the
  stack, through `stack-spike`; a developer packaging the app, through `cargo appimage`; a
  developer checking redb across processes, through `redb-process-probe`; a developer validating
  on-screen text, through `visual_validation`; no product crate depends on anything here.
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

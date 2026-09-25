**Status:** live

# README template: area root

**When to use:** the top of a code tree: `apps/`, `crates/` or `tools/`. An area root groups
several crates without being one. The [README standard](/documentation/standards/readme_standard.md)
defines every rule this template follows; the area root kind adds Getting started.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. A code README
carries no status line, so the block opens with the title.

````markdown
# <Name of the area, in plain words: no path, no backticks>

<One to three sentences: what the area is for and which crates it holds.>

## Contents

```text
<repository path of the folder>/
├── <crate folder>/  <the crate in one phrase: a lowercase phrase, no closing period>
└── <crate folder>/  <the crate in one phrase; entries run in case-insensitive name order>
```

## How it works

<How the crates fit together: which layer each sits on, who calls whom, and what they share. An
ASCII diagram in a text block helps here. Name each crate's part in one clause; its own README
holds the detail.>

## Getting started

<The few commands, run from the repository root, that build and test the area, each with what to
expect; say which stay in the foreground. Link the runbook for the full procedure.>

## Boundaries

- Depends on: <the other areas, crates.io crates and external programs the crates use>
- Used by: <the areas and crates outside that use these crates, found with git grep>
- Rules: <the invariants particular to the area, each with the gate or test that holds it; no
  repository-wide law restated>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

Written from `crates/`, its seven `Cargo.toml` files, the layer table in
`tools/repo_gates/src/layout.rs` and a `cargo test` run. The sample sits in a fenced block, so no
gate reads it as a README; the folder's own README.md is written from the same code and may differ.

````markdown
# Library crates

The product's library crates: the contracts between stages, child processes, media input,
subtitle formats, inference backends, the pipeline stages and the job runner. The app in `apps/`
composes them; nothing here opens a window or parses a command line.

## Contents

```text
crates/
├── child_process/     child processes with deadlines, process-group kills and drained pipes
├── inference/         the inference backends behind one trait per capability, and the model store
├── job_model/         the serde types every stage reads and writes: jobs, stage names, outputs, reports
├── media_io/          FFmpeg and ffprobe as child processes: probe, PCM streaming, shot changes
├── pipeline/          the job runner: stage order, resume, worker processes, progress
├── stages/            one module folder per pipeline stage, from probe to subtitle file
└── subtitle_formats/  the cue model, the SRT, WebVTT and ASS writers, and subtitle import
```

## How it works

Each crate sits on one layer and depends only on crates of a lower layer. `job_model` and
`child_process` form the bottom: the contract types every stage writes into a job's work
directory, and the one way to start an external program. `media_io`, `subtitle_formats` and
`inference` do the work of one kind each. `stages` holds one module folder per
[stage](/documentation/glossary.md#stage) and calls the three below it; `pipeline` runs the stages
of a job in order and starts each GPU stage as a
[worker process](/documentation/glossary.md#worker-process) of the app binary.

```text
layer 4   apps/tbd_subtitles
layer 3   pipeline
layer 2   stages
layer 1   media_io   subtitle_formats   inference
layer 0   job_model  child_process
```

`job_model` holds the stage names and `child_process` its runner; the other crates hold their
module headers, and their code is not written yet.

## Getting started

Run these from the repository root:

```bash
cargo build --workspace                     # builds every crate with the app and the tools
cargo test -p job_model -p child_process    # the unit tests: stage names, and 20 runner tests that start sh, cat and sleep
cargo gates crate-layering                  # each crate depends only on crates of a lower layer
```

## Boundaries

- Depends on: the crates.io crates `serde` (in `job_model`) and `libc` (in `child_process`); at
  run time, the programs the crate headers name: FFmpeg and ffprobe for `media_io`, and the
  `claude` CLI for `inference`.
- Used by: `apps/tbd_subtitles/`, which depends on `pipeline` and `job_model`; and
  `tools/verification_core/`, which runs `git` through `child_process`.
- Rules:
  - a crate depends only on crates of a lower layer, as the layer table in
    `tools/repo_gates/src/layout.rs` sets them (`cargo gates crate-layering`);
  - a crate starts no external program but FFmpeg, ffprobe and the `claude` CLI, always through
    `child_process`.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the processes, the crates
  and the job work directory.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does.
````

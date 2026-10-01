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

Written from `crates/`, its nine `Cargo.toml` files and the layer table `PRODUCT_LAYERS` in
`tools/repo_gates/src/layout.rs`. The sample sits in a fenced block, so no gate reads it as a
README; the folder's own README.md is written from the same code and may differ.

````markdown
# Library crates

The product's library crates: the contracts between stages, child processes, the worker channel,
the application icon, media input, subtitle formats, inference backends, the pipeline stages and
the job runner. The three app binaries in `apps/` compose them; nothing here opens a window or
parses a command line.

## Contents

```text
crates/
├── app_icon/          the application icon, painted in code as RGBA pixels at any square size
├── child_process/     external programs with deadlines, process-group kills and drained pipes
├── inference/         the model backends, the model store and the CUDA runtime lookup
├── job_model/         the contracts between stages: stage and step names, job record, outputs
├── media_io/          FFmpeg and ffprobe as child processes: probe, PCM, frames, localized video
├── pipeline/          the job runner: step order, resume, job store, workers, sign library, report
├── stages/            one module folder per pipeline stage, from probe to subtitle file
├── subtitle_formats/  the cue model, the SRT, WebVTT and ASS writers, and subtitle import
└── worker_channel/    the frames a worker and the job runner exchange on pipes, and the worker's side
```

## How it works

Each crate sits on one layer and depends only on crates of a lower layer. `job_model`,
`child_process`, `app_icon` and `worker_channel` form the bottom: the contract types every step
stores, the one way to start an external program, the icon's pixels and the framed messages
between a worker and the runner. `media_io`, `subtitle_formats` and `inference` do the work of one
kind each. `stages` holds one module folder per [stage](/documentation/glossary.md#stage) and
calls the three below it; `pipeline` runs a job's twenty-nine steps in order, keeps every step's
output and record in the job's one `job.redb`, and starts each model step as a
[worker process](/documentation/glossary.md#worker-process) of one of the three app binaries.

```text
layer 4   apps/tbd_subtitles   apps/tbd_subtitles_ggml   apps/tbd_subtitles_llm
layer 3   pipeline
layer 2   stages
layer 1   media_io   subtitle_formats   inference
layer 0   job_model  child_process  app_icon  worker_channel
```

## Getting started

Run these from the repository root:

```bash
cargo build --workspace                     # builds every crate with the apps and the tools
cargo test -p job_model -p child_process    # the unit tests of the two bottom-layer crates
cargo gates crate-layering                  # each crate depends only on crates of a lower layer
```

## Boundaries

- Depends on: crates.io crates such as `serde`, `rkyv`, `redb`, `ort` and `libc`; at run time,
  FFmpeg and ffprobe for `media_io`, and the `claude` CLI for `inference`.
- Used by: the three app binaries in `apps/`; the repository tools in `tools/`, each within the
  crates the tool table in `tools/repo_gates/src/layout.rs` allows it.
- Rules:
  - a crate depends only on crates of a lower layer, as `PRODUCT_LAYERS` in
    `tools/repo_gates/src/layout.rs` sets them (`cargo gates crate-layering`);
  - a crate starts no external program but FFmpeg, ffprobe, the `claude` CLI and the app's own
    worker binaries, always through `child_process`.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the processes, the crates
  and the job work directory.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does.
````

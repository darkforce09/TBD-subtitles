# Applications

The executables people run and the job runner starts. There are three binaries: `tbd-subtitles`,
which holds the desktop window, the headless command line and the
[worker processes](/documentation/glossary.md#worker-process) for every step on ONNX Runtime,
FFmpeg or the `claude` CLI; `tbd-subtitles-ggml`, the worker for the Whisper steps; and
`tbd-subtitles-llm`, the local visual translation worker. Separate processes isolate the native
inference runtimes and release GPU memory between stages.

## Contents

```text
apps/
├── tbd_subtitles_llm/   the isolated local visual translation worker
├── tbd_subtitles/       the `tbd-subtitles` binary: the window, the headless commands, the workers
└── tbd_subtitles_ggml/  the `tbd-subtitles-ggml` binary: the Whisper steps through CrispASR
```

## How it works

An application composes the library crates under `crates/` and adds only what a person or a
process launcher touches: the command line, the window and the error report at the top. The
pipeline logic, the media handling and the subtitle formats live in those crates, never here.

`tbd-subtitles process` runs a job through `crates/pipeline/`, whose job runner starts each
worker step as `tbd-subtitles worker <step>` or, for `asr_whisper` and `redecode_whisper`, as
`tbd-subtitles-ggml worker <step>`, or `tbd-subtitles-llm worker text_translate`. The runner finds
both workers beside the main binary, so all three share the target folder. Each crate's README describes its binary,
and `tbd_subtitles/src/README.md` maps the main binary's modules.

```text
apps/tbd_subtitles ──────┐
                         ├──▶ crates/pipeline ──▶ crates/stages ──▶ inference, media_io, subtitle_formats
apps/tbd_subtitles_ggml ─┤          │
apps/tbd_subtitles_llm ──┘          │
                                    └──▶ crates/job_model

job runner (crates/pipeline) ──▶ tbd-subtitles worker <step>        every other worker step
                            ├─▶ tbd-subtitles-ggml worker <step>   asr_whisper, redecode_whisper
                            └─▶ tbd-subtitles-llm worker <step>    text_translate
```

## Getting started

Run these from the repository root; the window needs a desktop session. The ggml worker's
feature build and its CUDA toolkit are in `tbd_subtitles_ggml/README.md`.

```bash
cargo run -p tbd_subtitles             # opens the window, empty queue; stays in the foreground
cargo run -p tbd_subtitles -- --help   # the subcommands: gui, process, worker
cargo test -p tbd_subtitles -p tbd_subtitles_ggml   # CLI, queue, rendering and architecture tests; headless
```

## Boundaries

- Depends on: `crates/pipeline/`, `crates/job_model/` and, for the built-in glossary,
  `crates/stages/`; CrispASR through `pipeline`'s `crispasr` feature in the ggml worker.
- Used by: people at a desktop or a terminal; the job runner in `crates/pipeline/` starts both
  binaries' `worker` command; no crate links an application.
- Rules: an application sits on the top layer, and no crate under `crates/` depends on one
  (`cargo gates crate-layering`); ggml and ONNX Runtime never share a binary, so the Whisper
  steps run only in `tbd-subtitles-ggml`; every folder here carries a README whose Contents block
  matches it (`cargo gates readme-coverage`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the processes, crates and
  work directory the binaries tie together.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why the Whisper steps have a binary of their own.
- [Coding standards](/documentation/standards/coding_standards.md) — the layering and the feature
  folder layout the app follows.

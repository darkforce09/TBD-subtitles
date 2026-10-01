# Stack spike source

The `stack-spike` command line, the context every item shares, the measurement of one item in a
worker process, the items themselves, and the report.

## Contents

```text
tools/stack_spike/src/
├── context.rs  the video, the work folder and its JSON files, the FFmpeg programs
├── fetch.rs    the `fetch` command: list, download and check the pinned models and runtime archives
├── items/      one module per stack item, each run inside a worker process
├── main.rs     the binary root: the command line and the dispatch to each command
├── measure/    the parent side (budget check, spawn, VRAM sampling, result file) and the worker side
├── report.rs   the `report` command: the results table and the 120-minute projection
└── wav.rs      16-bit WAV excerpts of a 16 kHz stem, for listening by ear
```

## How it works

`main.rs` builds a `Context` for `run`, `worker` and `report`. `run` hands each item to
`measure::measure`, which starts this binary again as `worker <item>` (or, for a ggml or local
language-model item, `stack-spike-ggml` or `stack-spike-llm` beside it); the worker calls
`Item::run` and writes its own times and memory peaks, and the parent adds the wall time and the
VRAM peaks. `report` reads every `results/<item>.json` back.

## Boundaries

- Depends on: `inference`, `media_io`, `stages`, `job_model`, `child_process` and
  `pipeline::measure`; `clap`, `anyhow`, `serde` and `serde_json`.
- Used by: nothing; it is the binary's source.
- Rules: a command that could not run exits non-zero with the reason, never a success (the
  header in `main.rs`).

# Stack spike

The `stack_spike` binary (`stack-spike`): the measuring harness that proves each piece of the ML
stack on one real video before the pipeline is built around it. It downloads the pinned model
files and the CUDA 13 runtime, runs each stack item in a worker process of its own, and records
wall time, speed against realtime, peak VRAM, peak RAM and quality notes. A developer runs it on
the host.

## Contents

```text
tools/stack_spike/
├── Cargo.toml  the `stack_spike` binary package and the reason for each dependency
└── src/        the command line, the work-folder context, the measurement, the items and the report
```

## How it works

```text
stack-spike fetch ──▶ inference::model_store ──▶ models/<model>/ and runtime/<folder>/
stack-spike run [items] --video V
   └─ per item: GPU budget check (NVML) ──▶ spawn `stack-spike worker <item>` (CUDA runtime on
      LD_LIBRARY_PATH) ──▶ NVML samples the worker every 100 ms, its stdout is forwarded
      └─ worker: item.run(ctx) ──▶ work/<outputs> + results/<item>.worker.json (times, VmHWM,
         the largest child's ru_maxrss, notes)
   └─ parent: results/<item>.json and results/<item>.log
stack-spike report --video V ──▶ Markdown table + the 120-minute projection per item
```

Items run in the order `Item::ALL` lists them, and later items read what earlier ones wrote to
the work folder (`probe.json`, `mix_16k.f32`, the stems `vocals_16k.<separator>.f32` and
`background_16k.<separator>.f32`, …). The separation items also write 30-second WAV excerpts of
the mix and both stems for listening. The work folder defaults to
`<data home>/tbd-subtitles/work/spike-<video name as a slug>`; the video is only read.

## Getting started

Run these from the repository root; the measuring commands run on the host, where FFmpeg 8.1,
the NVIDIA driver and NVML are:

```bash
cargo run -p stack_spike -- fetch --dry-run   # every pinned file with its size and state
cargo run --release -p stack_spike -- fetch   # download and check what is missing
cargo build --release -p stack_spike
distrobox-host-exec target/release/stack-spike run decode shots --video "<video>"
distrobox-host-exec target/release/stack-spike report --video "<video>"
```

A GPU item refuses to run (and records "not run") while less than 5632 MiB of VRAM is free.

## Configuration

- `XDG_DATA_HOME`, else `HOME`: where the models, runtime and work folders live (read by
  `crates/inference/src/model_store/mod.rs`).
- `--work <dir>`: another work folder (`src/main.rs`).

## Public surface

- The binary `stack-spike` with the commands `fetch [--only <id>]... [--dry-run]`,
  `run [<item>]... --video <path> [--work <dir>]` and `report --video <path> [--work <dir>]`; the
  hidden `worker` command is for `run` alone.
- No library.

## Boundaries

- Depends on: `crates/child_process/` (workers), `crates/inference/` (model store, CUDA runtime,
  models), `crates/job_model/`, `crates/media_io/` and `crates/stages/` (the items under test); `clap`, `anyhow`, `serde`,
  `serde_json`, `libc` and `nvml-wrapper`; the programs `ffmpeg` and `ffprobe` through `media_io`.
- Used by: a developer measuring the stack; nothing depends on it.
- Rules:
  - the tool depends only on the workspace crates the tool table in
    `tools/repo_gates/src/layout.rs` lists for it (`cargo gates crate-layering`);
  - an item that could not run is recorded as not run or failed with the reason, never with
    numbers, and the video and its folder are only read (the headers in `src/main.rs` and
    `src/measure/mod.rs`).

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the stack items the tool measures.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the crates and model files under
  test.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA runtime the tool fetches.

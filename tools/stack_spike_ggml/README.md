# Stack spike ggml worker

The `stack_spike_ggml` binary (`stack-spike-ggml`): the stack spike's worker for the ggml models,
Whisper large-v3 and large-v3-turbo through CrispASR. It is a binary of its own because ggml and
ONNX Runtime corrupt each other's heap when loaded into one process; `stack-spike run` starts it
for the ggml items and measures it like any other worker.

## Contents

```text
tools/stack_spike_ggml/
├── build.rs    with `crispasr`, the rpath to the libcrispasr and ggml libraries crispasr-sys built
├── Cargo.toml  the binary package, its `crispasr` feature and the reason for each dependency
└── src/        the `worker` command, the ggml items and the worker report
```

## How it works

`stack-spike` finds `stack-spike-ggml` beside its own binary and starts it as
`stack-spike-ggml worker <item> --video <path> --work <dir>`, with the CUDA runtime on
`LD_LIBRARY_PATH`. The worker runs the item over the work folder's chunk plan and writes
`results/<item>.worker.json` in the shape `stack-spike` reads, so the parent adds the wall time
and the VRAM peaks as for its own items. Without the `crispasr` feature the binary builds and
refuses every item with that reason.

## Getting started

Build it with the feature, with the CUDA toolkit from the runtime folder (see the development
environment runbook), then run the items through `stack-spike`:

```bash
cargo build -p stack_spike_ggml   # without the feature: builds, and refuses every item
```

## Configuration

- The Cargo feature `crispasr` links CrispASR and ggml with CUDA (`Cargo.toml`); building it needs
  cmake and nvcc, with `CUDACXX`, `CUDAToolkit_ROOT` and `CUDAARCHS` set.
- `LD_LIBRARY_PATH` must hold the CUDA 13 `lib/` folder at run time; `stack-spike` sets it.

## Public surface

- The binary `stack-spike-ggml` with the command `worker <item> --video <path> --work <dir>`, for
  `stack-spike` alone.
- No library.

## Boundaries

- Depends on: `crates/inference/` (`ggml::crispasr`, the models folder), `crates/stages/` (`asr`),
  `crates/job_model/`; `crispasr-sys` for its build metadata; `clap`, `anyhow`, `serde`,
  `serde_json`.
- Used by: `tools/stack_spike/`, which starts it for the ggml items.
- Rules:
  - the tool depends only on the workspace crates the tool table in
    `tools/repo_gates/src/layout.rs` lists for it (`cargo gates crate-layering`);
  - it never loads ONNX Runtime (the header in `src/main.rs`).

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA toolkit the feature build needs.

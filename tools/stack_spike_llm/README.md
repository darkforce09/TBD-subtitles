# Stack spike LLM worker

The `stack_spike_llm` binary (`stack-spike-llm`): the stack spike's worker for the local language
model, Qwen3.5-4B through mistral.rs. It is a binary of its own because candle's CUDA runtime and
ONNX Runtime corrupt each other's heap when loaded into one process; `stack-spike run` starts it
for the `llm-local` item and measures it like any other worker.

## Contents

```text
tools/stack_spike_llm/
├── Cargo.toml  the binary package, its `mistralrs` feature and the reason for each dependency
└── src/        the `worker` command and the worker report
```

## How it works

`stack-spike` finds `stack-spike-llm` beside its own binary and starts it as
`stack-spike-llm worker llm-local --video <path> --work <dir>`, with the CUDA runtime on
`LD_LIBRARY_PATH`. The worker reads the diff sheet and the glossary the `diff-sheet` item wrote,
adjudicates with the local model, checks the answer, writes `adjudicated.qwen3.5-4b.json` and
`results/llm-local.worker.json`. Without the `mistralrs` feature it builds and refuses the item.

## Getting started

Build it with the feature as the development environment runbook's step 10 says, then run the
item through `stack-spike`:

```bash
cargo build -p stack_spike_llm   # without the feature: builds, and refuses the item
```

## Configuration

- The Cargo feature `mistralrs` links mistral.rs with CUDA (`Cargo.toml`); building it needs the
  CUDA 13.3 compiler and the 13.4 libraries on `LIBRARY_PATH`.
- `LD_LIBRARY_PATH` must hold the CUDA 13.4 `lib/` folder at run time; `stack-spike` sets it.

## Public surface

- The binary `stack-spike-llm` with the command `worker llm-local --video <path> --work <dir>`, for
  `stack-spike` alone.
- No library.

## Boundaries

- Depends on: `crates/inference/` (`llm::mistral_rs`, the models folder), `crates/stages/`
  (`adjudication`, `diff_sheet`), `crates/job_model/`; `clap`, `anyhow`, `serde`, `serde_json`.
- Used by: `tools/stack_spike/`, which starts it for `llm-local`.
- Rules:
  - the tool depends only on the workspace crates the tool table in
    `tools/repo_gates/src/layout.rs` lists for it (`cargo gates crate-layering`);
  - it never loads ONNX Runtime or ggml (the header in `src/main.rs`).

## Related documentation

- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the CUDA 13.3 compiler the feature build needs.

# Inference backends

The `inference` crate: the backends that run the models the GPU stages need (ONNX Runtime, ggml
and candle models and the language-model backends), the model store that downloads and verifies
model files and the CUDA 13 runtime archives, and the lookup of that runtime for GPU workers.

## Contents

```text
crates/inference/
├── Cargo.toml  the `inference` library package and the reason for each dependency
└── src/        the runtime backends, the language models, the model store and the CUDA runtime lookup
```

## How it works

The crate is for the stages that run in a [worker process](/documentation/glossary.md#worker-process):
`stages` calls it from inside that process, which loads each model once and exits when the stage
is done. The modules divide the work by runtime: `onnx` for ONNX Runtime on CUDA, `ggml` for the
ggml-based crates, `candle` for the pure-Rust engine, `llm` for the language models that
adjudicate the [diff sheet](/documentation/glossary.md#diff-sheet). `model_store` downloads every
model file and runtime archive the manifest pins, checking each SHA-256 before the file is used,
and `cuda_runtime` finds the unpacked CUDA 13 libraries and gives the `LD_LIBRARY_PATH` a GPU
worker starts with. `src/README.md` describes each module.

```text
stack-spike fetch ──▶ model_store ──▶ ~/.local/share/tbd-subtitles/models/<model>/
                                  └─▶ ~/.local/share/tbd-subtitles/runtime/{cuda-13.4,cudnn-9.26}/
worker spawn ──▶ cuda_runtime::CudaRuntime::locate ──▶ worker_env (LD_LIBRARY_PATH)
```

## Getting started

Run these from the repository root:

```bash
cargo build -p inference   # the model store and the runtime lookup
cargo test -p inference    # unit tests for the pins, downloads in place and the runtime lookup
```

## Configuration

- `XDG_DATA_HOME`, else `HOME`: the data folder `tbd-subtitles/` holding `models/` and `runtime/`
  (`src/model_store/mod.rs`); one of them must be set.
- `LD_LIBRARY_PATH`: kept after the runtime folders in a GPU worker's environment
  (`src/cuda_runtime/mod.rs`).

## Public surface

- The library `inference`, with the public modules `candle`, `cuda_runtime`, `ggml`, `llm`
  (holding `llm::claude_cli` and `llm::mistral_rs`), `model_store` and `onnx`.
- No binary.

## Boundaries

- Depends on: `ureq`, `sha2`, `lzma-rs` and `tar` for the model store; `child_process` and
  `job_model`, declared for the backends.
- Used by: `tools/stack_spike/`; `crates/stages/` declares it as a dependency.
- Rules:
  - the crate sits in layer 1 and depends only on layer 0 crates (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - models are downloaded already exported, from pinned URLs with checksums, and never converted,
    and no two crates that bundle ggml link into one binary (the crate header in
    `crates/inference/src/lib.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#recommended-stack) — the crates and
  model files for each capability.
- [Decisions](/documentation/decisions.md) — native runtimes where no pure-Rust engine competes,
  and one worker process per GPU stage.
- [System overview](/documentation/architecture/system_overview.md#models) — the models folder and
  its manifest.

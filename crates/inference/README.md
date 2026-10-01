# Inference backends

The `inference` crate: the backends that run the models the GPU stages need (ONNX Runtime, ggml
and candle models, local OCR and the language-model backends), the model store that downloads and
verifies model files and the CUDA 13 runtime archives, and the lookup of that runtime for GPU
workers.

## Contents

```text
crates/inference/
├── Cargo.toml  the `inference` library package and the reason for each dependency
└── src/        the runtime backends, the language models, the model store and the CUDA runtime lookup
```

## How it works

The crate is for the stages that run in a [worker process](/documentation/glossary.md#worker-process):
`stages` and the step tasks in `pipeline` call it from inside that process, which loads each model
once and exits when the step is done. The modules divide the work by runtime: `onnx` for ONNX
Runtime on CUDA, `ggml` for the ggml-based crates, `candle` for the pure-Rust engine, `ocr` for
PP-OCRv5 and manga-ocr on the same ONNX Runtime, `llm` for the language models that adjudicate
the [diff sheet](/documentation/glossary.md#diff-sheet), translate on-screen text and run Fix It.
`model_store` downloads every model file and runtime archive the manifest pins, checking each
SHA-256 before the file is used, and `cuda_runtime` finds the unpacked CUDA 13 libraries and
gives the `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` a GPU worker starts with. `src/README.md`
describes each module.

```text
Settings downloads, stack-spike fetch ──▶ model_store ──▶ ~/.local/share/tbd-subtitles/models/<model>/
                   └─▶ ~/.local/share/tbd-subtitles/runtime/{cuda-13.4,cudnn-9.26,onnxruntime-1.28.2}/
worker spawn ──▶ cuda_runtime::CudaRuntime::locate ──▶ worker_env (LD_LIBRARY_PATH, ORT_DYLIB_PATH)
```

## Getting started

Run these from the repository root:

```bash
cargo build -p inference   # the backends, the model store and the runtime lookup
cargo test -p inference    # unit tests; the model checks are #[ignore] and run on the host
```

## Configuration

- `XDG_DATA_HOME`, else `HOME`: the data folder `tbd-subtitles/` holding `models/` and `runtime/`
  (`src/model_store/mod.rs`); one of them must be set.
- `LD_LIBRARY_PATH`: kept after the runtime folders in a GPU worker's environment
  (`src/cuda_runtime/mod.rs`).
- The Cargo feature `crispasr` (off by default): CrispASR and ggml with CUDA for `ggml::crispasr`;
  building it needs cmake and the CUDA toolkit (`Cargo.toml`).
- The Cargo feature `mistralrs` (off by default): mistral.rs with CUDA for `llm::mistral_rs`;
  building it needs the CUDA 13.3 compiler and the 13.4 libraries on `LIBRARY_PATH`
  (`Cargo.toml`).
- `ORT_DYLIB_PATH`: where `ort` loads ONNX Runtime from; set for a GPU worker by
  `CudaRuntime::worker_env` (`src/cuda_runtime/mod.rs`) and read by the `ort` crate.
- `HOME`: `$HOME/.local/bin/claude` is the fallback when `claude` is on no `PATH` entry
  (`src/llm/claude_cli/mod.rs`).
- `TBD_CLAUDE_CALL_LIMIT`: the cap on `claude` calls at once across processes that is written
  when no limit is saved yet, 32 when unset; the saved limit is `claude-slots/limit.json` in the
  data folder, written through a `.part` file and renamed (`src/llm/claude_cli/shared_slots.rs`).

## Public surface

- The library `inference`, with the public modules `candle`, `cuda_runtime`, `ggml`, `llm`
  (holding `llm::call_gate`, `llm::call_log`, `llm::claude_cli` and `llm::mistral_rs`),
  `model_store`, `ocr` and `onnx`.
- No binary.

## Boundaries

- Depends on: `ort` (ONNX Runtime, loaded at run time), `realfft`, `parakeet-rs` and `tokenizers`
  for the ONNX backends; `oar-ocr` and `image` for OCR; `crispasr` (git tag `v0.8.37`, optional)
  for the ggml backend; `mistralrs` (git tag `v0.9.4`) and `tokio` (optional) for the local
  language model; `serde_json`; `ureq`, `sha2`, `lzma-rs`, `flate2` and `tar` for the model
  store; `child_process` for the `claude` CLI; `job_model` for timed words, on-screen geometry
  and a model call's record; `tracing` for each call's summary and exchange.
- Used by: `crates/stages/`, `crates/pipeline/`, `apps/tbd_subtitles/`,
  `apps/tbd_subtitles_llm/`, `tools/appimage_builder/`, `tools/stack_spike/`,
  `tools/stack_spike_ggml/`, `tools/stack_spike_llm/` and `tools/visual_validation/`.
- Rules:
  - the crate sits in layer 1 and depends only on layer 0 crates (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - models are downloaded already exported, from pinned URLs with checksums, and never converted,
    and no two crates that bundle ggml link into one binary (the crate header in
    `crates/inference/src/lib.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#recommended-stack) — the crates and
  model files for each capability.
- [Decisions](/documentation/decisions/) — native runtimes where no pure-Rust engine competes,
  and one worker process per GPU stage.
- [System overview](/documentation/architecture/system_overview.md#models) — the models folder and
  its manifest.

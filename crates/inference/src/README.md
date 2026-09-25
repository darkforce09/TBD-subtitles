# Inference source

The `inference` library: one module folder per inference runtime, one for the language-model
backends, the model store that downloads pinned model files and runtime archives, and the lookup
of the CUDA 13 runtime the GPU backends load.

## Contents

```text
crates/inference/src/
├── candle/         models run through candle, the pure-Rust engine, where it is competitive
├── cuda_runtime/   the CUDA 13 and cuDNN libraries on disk and the environment a GPU worker needs
├── ggml/           models run through ggml-based crates, each in a worker process of its own
├── lib.rs          the crate root: the module list and the crate header
├── llm/            the language-model backends that adjudicate the diff sheet
├── model_store/    the models and runtime folders: the pinned manifest, downloads, archive unpacking
└── onnx/           models run through ONNX Runtime on CUDA: recognition, separation, alignment, events
```

## How it works

Pure-Rust engines come first: `candle` and the `mistral_rs` backend under `llm/`. `onnx/` and
`ggml/` are for the models where a crate that binds a native runtime is faster or more accurate.
`model_store/` fetches every model file, in
[GGUF, ONNX or safetensors](/documentation/glossary.md#gguf-onnx-safetensors) form, and the CUDA
13 runtime archives, checking each against its pinned SHA-256 before any backend loads it.
`cuda_runtime/` finds the unpacked runtime (beside the executable, or in the user runtime folder)
and gives the `LD_LIBRARY_PATH` a GPU worker is started with.

## Public surface

- `model_store`: `models_dir`, `runtime_dir`, `fetch_model`, `is_complete`, `install_archive`,
  the `manifest` with `MODEL_FILES` and `CUDA_ARCHIVES`, and `StoreError`; for
  `tools/stack_spike/`.
- `cuda_runtime`: `CudaRuntime::locate` and `CudaRuntime::worker_env`; for the processes that
  start GPU workers.
- `candle`, `ggml`, `llm` and `onnx`: public modules for the GPU stages in `crates/stages/src/`.

## Boundaries

- Depends on: `ureq`, `sha2`, `lzma-rs` and `tar` in `model_store/`; the crate declares
  `child_process` and `job_model` for the backends.
- Used by: `tools/stack_spike/`; `crates/stages/` declares the crate as a dependency.
- Rules: two modules that bundle ggml never link into one binary, so each ggml crate runs in a
  worker process of its own (the headers in `lib.rs` and `ggml/mod.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#hard-gaps) — the gaps and native-runtime
  clashes these modules work around.

# Inference source

The `inference` library: one module folder per inference runtime, one for the language-model
backends, the model store that downloads pinned model files and runtime archives, and the lookup
of the CUDA 13 runtime the GPU backends load.

## Contents

```text
crates/inference/src/
├── candle/         models run through candle, the pure-Rust engine, where it is competitive
├── cuda_runtime/   the CUDA 13 and cuDNN libraries on disk and the environment a GPU worker needs
├── ggml/           Whisper and the Qwen3 aligner through CrispASR, behind the `crispasr` feature
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
13 and ONNX Runtime archives, checking each against its pinned SHA-256 before any backend loads
it.
`cuda_runtime/` finds the unpacked runtime (beside the executable, or in the user runtime folder)
and gives the `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` a GPU worker is started with. `onnx/`
holds the ONNX Runtime session helper, the separation models and Parakeet-TDT; `ggml/` holds
CrispASR's Whisper and Qwen3 aligner, built only with the `crispasr` feature and never in a
binary that loads ONNX Runtime.

## Public surface

- `model_store`: `models_dir`, `runtime_dir`, `fetch_model`, `is_complete`, `install_archive`,
  `is_archive_installed`, the `manifest` with `MODEL_FILES` and `CUDA_ARCHIVES`, and `StoreError`;
  for `tools/stack_spike/` and the app's models view.
- `cuda_runtime`: `CudaRuntime::locate` and `CudaRuntime::worker_env`; for the processes that
  start GPU workers.
- `onnx`: `session::open`, `Device`, `OnnxError`, `separation::{MdxNet, MelRoformer,
  OverlapAdd, WindowModel}` and `parakeet_tdt::ParakeetTdt`; for `crates/stages/` and
  `tools/stack_spike/` and the app's models view.
- `ggml::crispasr::{Whisper, align_qwen3}` with the `crispasr` feature; for `crates/stages/` and
  `tools/stack_spike_ggml/`.
- `candle` and `llm`: public modules for the GPU stages in `crates/stages/src/`.

## Boundaries

- Depends on: `ort`, `realfft` and `parakeet-rs` in `onnx/`; `crispasr` in `ggml/` (optional); `ureq`, `sha2`, `lzma-rs`, `flate2` and `tar` in
  `model_store/`; the crate declares `child_process` and `job_model` for the backends.
- Used by: `crates/stages/`, `tools/stack_spike/` and `tools/stack_spike_ggml/`.
- Rules: two modules that bundle ggml never link into one binary, so each ggml crate runs in a
  worker process of its own (the headers in `lib.rs` and `ggml/mod.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#hard-gaps) — the gaps and native-runtime
  clashes these modules work around.

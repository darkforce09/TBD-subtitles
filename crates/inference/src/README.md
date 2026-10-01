# Inference source

The `inference` library: one module folder per inference runtime, one for local OCR, one for the
language-model backends, the model store that downloads pinned model files and runtime archives,
and the lookup of the CUDA 13 runtime the GPU backends load.

## Contents

```text
crates/inference/src/
├── candle/         models run through candle, the pure-Rust engine, where it is competitive
├── cuda_runtime/   the CUDA 13 and cuDNN libraries on disk and the environment a GPU worker needs
├── ggml/           Whisper and the Qwen3 aligner through CrispASR, behind the `crispasr` feature
├── lib.rs          the crate root: the module list and the crate header
├── llm/            the language-model backends, their shared call cap and their call log
├── model_store/    the models and runtime folders: the pinned manifest, downloads, archive unpacking
├── ocr/            PP-OCRv5 and manga-ocr through the pinned ONNX runtime
└── onnx/           ONNX Runtime on CUDA: recognition, separation, alignment, events, inpainting
```

## How it works

Pure-Rust engines come first: `candle` and the `mistral_rs` backend under `llm/`. `onnx/`,
`ocr/` and `ggml/` are for the models where a crate that binds a native runtime is faster or more
accurate. `model_store/` fetches every model file, in
[GGUF, ONNX or safetensors](/documentation/glossary.md#gguf-onnx-safetensors) form, and the CUDA
13 and ONNX Runtime archives, checking each against its pinned SHA-256 before any backend loads
it.
`cuda_runtime/` finds the unpacked runtime (beside the executable, or in the user runtime folder)
and gives the `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` a GPU worker is started with. `onnx/`
holds the ONNX Runtime session helper, the separation models, Parakeet-TDT and Parakeet-CTC, CED
and LaMa; `ocr/` runs PP-OCRv5 detection and recognition with a manga-ocr second reading on the
same ONNX Runtime, and the detector pool that screens full-resolution frames on CUDA or
TensorRT; `ggml/` holds CrispASR's Whisper and Qwen3 aligner, built only with the `crispasr`
feature and never in a binary that loads ONNX Runtime. `llm/` holds the `claude` CLI
and mistral.rs backends, the call gate Fix It's runs share, and the call log.

## Public surface

- `model_store`: `app_data_dir`, `models_dir`, `runtime_dir`, `fetch_model`, `is_complete`,
  `install_archive`, `is_archive_installed`, `install_libraries`, `are_libraries_installed`,
  `fetch_verified`, `sha256_of`, the `manifest` with `MODEL_FILES`, `CUDA_ARCHIVES`,
  `ONNX_RUNTIME_ARCHIVE`, `TENSORRT_ARCHIVE`, `TENSORRT_LIBRARIES` and `TENSORRT_VERSION`, and
  `StoreError`; for the app, `crates/pipeline/`, `crates/stages/` and the tools.
- `cuda_runtime`: `CudaRuntime::locate`, `CudaRuntime::worker_env`,
  `CudaRuntime::tensorrt_available` and `tensorrt_version`, and `REQUIRED_CUDA_LIBS`; for the
  processes that start GPU workers, the app's settings and the AppImage builder.
- `onnx`: `session::open`, `Device`, `OnnxError`, `separation::{MdxNet, MelRoformer,
  OverlapAdd, WindowModel}`, `parakeet_tdt::ParakeetTdt`, `parakeet_ctc::ParakeetCtc`,
  `ced::Ced` and `lama::Lama`; for `crates/stages/`, `crates/pipeline/` and `tools/stack_spike/`.
- `ocr`: `DetectorPool`, `PoolOptions`, `SearchMode`, the `pool` contract (`TextScreening` and
  its jobs), `TextDetection`, `OcrDetector`, `OcrReader` and `OcrError`; for
  `crates/stages/src/onscreen_text/`, `crates/pipeline/` and `tools/visual_validation/`.
- `ggml::crispasr::{Whisper, align_qwen3}` with the `crispasr` feature; for `crates/stages/`,
  `crates/pipeline/` and `tools/stack_spike_ggml/`.
- `llm`: `LanguageModel`, `Completion`, `LlmError` and `purpose`, with `claude_cli`,
  `mistral_rs`, `call_gate` and `call_log`; for `crates/stages/`, `crates/pipeline/`, the app and
  the stack spike tools.
- `candle`: a public module with no items yet.

## Boundaries

- Depends on: `ort`, `realfft`, `parakeet-rs` and `tokenizers` in `onnx/`; `ort`, `oar-ocr`,
  `ndarray`, `rayon`, `sha2` and `image` in `ocr/`; `crispasr` in `ggml/` (optional);
  `child_process` in `llm/claude_cli/`; `mistralrs` and `tokio` in `llm/mistral_rs/` (optional); `ureq`, `sha2`, `lzma-rs`, `flate2`
  and `tar` in `model_store/`; `job_model` for timed words, on-screen geometry and a model call's
  record; `tracing`.
- Used by: `crates/stages/`, `crates/pipeline/`, `apps/tbd_subtitles/`,
  `apps/tbd_subtitles_llm/`, `tools/appimage_builder/`, `tools/stack_spike/`,
  `tools/stack_spike_ggml/`, `tools/stack_spike_llm/` and `tools/visual_validation/`.
- Rules: two modules that bundle ggml never link into one binary, so each ggml crate runs in a
  worker process of its own (the headers in `lib.rs` and `ggml/mod.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#hard-gaps) — the gaps and native-runtime
  clashes these modules work around.

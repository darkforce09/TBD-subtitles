# Inference source

The `inference` library: one module folder per inference runtime, one for the language-model
backends, and one for the model store. The modules are not written yet.

## Contents

```text
crates/inference/src/
├── candle/       models run through candle, the pure-Rust engine, where it is competitive
├── ggml/         models run through ggml-based crates, each in a worker process of its own
├── lib.rs        the crate root: the module list and the crate header
├── llm/          the language-model backends that adjudicate the diff sheet
├── model_store/  the models folder: the pinned manifest, downloads, and checks of files present
└── onnx/         models run through ONNX Runtime on CUDA: recognition, separation, alignment, events
```

## How it works

Pure-Rust engines come first: `candle` and the `mistral_rs` backend under `llm/`. `onnx/` and
`ggml/` are for the models where a crate that binds a native runtime is faster or more accurate.
`model_store/` is for fetching every model file, in
[GGUF, ONNX or safetensors](/documentation/glossary.md#gguf-onnx-safetensors) form, before any
backend loads it. Each module holds only its header; the backend traits are not written yet.

## Public surface

- `candle`, `ggml`, `llm`, `model_store` and `onnx`: public modules with no items yet, for the GPU
  stages in `crates/stages/src/`.

## Boundaries

- Depends on: nothing yet; the crate declares `child_process` and `job_model` for these modules.
- Used by: nothing yet; `crates/stages/` declares the crate as a dependency.
- Rules: two modules that bundle ggml never link into one binary, so each ggml crate runs in a
  worker process of its own (the headers in `lib.rs` and `ggml/mod.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#hard-gaps) — the gaps and native-runtime
  clashes these modules work around.

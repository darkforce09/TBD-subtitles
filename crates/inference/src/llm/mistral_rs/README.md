# Mistral.rs backend

A local language model through mistral.rs (candle, pure Rust) on the GPU: a GGUF model from the
models folder answers each request with JSON constrained to the request's schema. Built only with
the `mistralrs` feature.

## Contents

```text
crates/inference/src/llm/mistral_rs/
└── mod.rs  `MistralRs::open` (GGUF with its tokenizer and chat template) and `complete_json`
```

## Boundaries

- Depends on: `mistralrs` from the pinned git tag `v0.9.4` with `cuda` (Qwen3.5 GGUF support is
  not on crates.io), `tokio` to drive its async API, `serde_json`.
- Used by: the on-screen text translation in `crates/pipeline/src/tasks/onscreen.rs`, for the
  occurrences Claude leaves (behind the pipeline feature `mistralrs`, which the local-model worker
  `apps/tbd_subtitles_llm/` turns on), and `tools/stack_spike_llm/`, with the feature on.
- Rules:
  - answers are schema-constrained, temperature 0, thinking off; an answer that is not JSON is an
    `LlmError` (review);
  - its kernels build with nvcc 13.3 (mistral.rs refuses newer toolkits) and link against the
    13.4 runtime libraries; see the development environment runbook.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#6-language-models) — mistral.rs and the
  models that fit the GPU.
- [Development environment](/documentation/runbooks/development_environment.md#cuda-libraries-for-onnx-runtime)
  — the two CUDA toolkits and how to keep their builds apart.

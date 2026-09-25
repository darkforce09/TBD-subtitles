# CrispASR

Whisper transcription and the Qwen3 forced aligner through CrispASR, which runs them on ggml with
CUDA. Built only with the `crispasr` feature.

## Contents

```text
crates/inference/src/ggml/crispasr/
└── mod.rs  `Whisper::open` and `transcribe` (English, words with probabilities), `align_qwen3`
```

## Boundaries

- Depends on: the `crispasr` crate from the pinned git tag `v0.8.37` (its crates.io package ships
  no C++ sources), built with cmake and nvcc; `job_model::outputs::TimedWord`.
- Used by: `crates/stages/src/asr/engines.rs` (behind the stages feature `crispasr`) and
  `tools/stack_spike_ggml/`.
- Rules:
  - ggml and ONNX Runtime corrupt each other's heap in one process, so a binary that links this
    module never loads ONNX Runtime (the header in `tools/stack_spike_ggml/src/main.rs`);
  - CrispASR's session API gives Whisper no initial prompt, so the series glossary cannot be
    handed to Whisper here (review).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#4-forced-alignment) — the Qwen3 aligner.

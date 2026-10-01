# Parakeet-TDT

NVIDIA Parakeet-TDT-0.6B-v2, the backbone speech engine, through the parakeet-rs crate on this
crate's ONNX Runtime: 16 kHz mono chunks in, punctuated and cased English words with times out.

## Contents

```text
crates/inference/src/onnx/parakeet_tdt/
├── mod.rs  `ParakeetTdt::open` and `transcribe`, and `attach_punctuation`
└── tests/  unit tests for joining punctuation tokens to the word before
```

## Boundaries

- Depends on: `parakeet-rs` 0.3.8 (feature extraction, TDT decoding, word grouping) with
  `load-dynamic` and `cuda`, sharing this crate's `ort`; `job_model::outputs::TimedWord`.
- Used by: `crates/stages/src/asr/engines.rs`, the speech step in
  `crates/pipeline/src/tasks/speech.rs`, the model list in `crates/pipeline/src/models/mod.rs`,
  and `tools/stack_spike/`.
- Rules:
  - parakeet-rs puts the CPU provider after CUDA without `error_on_failure`, so a CUDA failure
    would fall back silently; the stack spike's VRAM sampler is what shows it ran on the GPU;
  - Parakeet emits punctuation as tokens of their own; each joins the word before
    (`punctuation_tokens_join_the_word_before`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#1-speech-recognition) — Parakeet and the
  other engines.

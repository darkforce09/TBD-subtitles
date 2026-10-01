# CED sound tagger

CED-base, an AudioSet tagger distilled from an ensemble, on ONNX Runtime: batches of 16 kHz audio
windows in, a probability for each of the 527 AudioSet classes out.

## Contents

```text
crates/inference/src/onnx/ced/
└── mod.rs  `Ced::open` and `scores`: equal-length windows ─▶ 527 probabilities each
```

## Boundaries

- Depends on: `crate::onnx::session` (CUDA), `ort`.
- Used by: `crates/stages/src/sound_events/` (as a `Tagger`), the sound-event step in
  `crates/pipeline/src/tasks/sounds.rs`, the model list in `crates/pipeline/src/models/mod.rs`,
  and `tools/stack_spike/`.
- Rules: the export holds its own feature extraction and its final sigmoid, so its output (named
  `logits`) is used as probabilities unchanged (review; the stack spike's notes showed a second
  sigmoid flattening every class to 0.5).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#5-sound-events) — CED and the other
  taggers.

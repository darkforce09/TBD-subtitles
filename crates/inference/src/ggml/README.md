# Ggml models

Models run through ggml-based crates. Each such crate bundles its own ggml, so only one links into
a binary, and that binary never loads ONNX Runtime: the two corrupt each other's heap in one
process. CrispASR is the one used, for Whisper and the Qwen3 forced aligner.

## Contents

```text
crates/inference/src/ggml/
├── crispasr/  Whisper and the Qwen3 aligner through CrispASR, behind the `crispasr` feature
└── mod.rs     the module list
```

## Boundaries

- Depends on: `crispasr` (git tag `v0.8.37`), only with the `crispasr` feature.
- Used by: `crates/stages/`, `crates/pipeline/` (the ggml worker's Whisper step) and
  `tools/stack_spike_ggml/`, with the feature on.
- Rules: no two ggml-bundling crates in one binary, and no ggml in a binary that loads ONNX
  Runtime (the headers in `lib.rs` and `ggml/mod.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#hard-gaps) — the ggml clashes.

# Local text recognition

Already exported PP-OCRv5 and manga-ocr models, loaded in isolated ONNX workers.

## Contents

```text
crates/inference/src/ocr/
├── manga.rs  bounded encoder/decoder beam search
└── mod.rs    detector, recognizer and strict CUDA setup
```

## How it works

PP-OCRv5 detects quadrilaterals and reads their crops. Vertical and uncertain crops receive a second manga-ocr reading. Disagreement remains uncertain. All files come from the pinned model store. The decoder has bounded beam width and sequence length.

CUDA uses heuristic convolution selection with a 2 GB arena limit and bounded convolution
workspace. Detector input dimensions and confidence thresholds remain fixed across frames.

## Boundaries

- Depends on: `ort`, `oar-ocr`, `image` and `job_model`.
- Used by: the visual stages in `stages::onscreen_text`.
- Rules: an unavailable CUDA provider is an error; no exported model is converted.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md) — runtime selection.

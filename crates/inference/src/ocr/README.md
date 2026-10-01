# Local text recognition

Already exported PP-OCRv5 and manga-ocr models, loaded in isolated ONNX workers.

## Contents

```text
crates/inference/src/ocr/
├── manga.rs  bounded encoder/decoder beam search
└── mod.rs    `TextDetection`, the two detectors (`OcrDetector`), `OcrReader` and strict CUDA setup
```

## How it works

The mobile PP-OCRv5 detector screens proxy frames in batches; the server detector confirms quadrilaterals on full-resolution stills, and the server recognizer reads their crops. Vertical and uncertain crops receive a second manga-ocr reading. Disagreement remains uncertain. `read_primary` returns the server recognizer's reading alone, Japanese and Latin alike, for the read-back check of lettered English, which the Japanese-only manga-ocr must not read. All files come from the pinned model store. The decoder has bounded beam width and sequence length.

CUDA uses heuristic convolution selection with a 3 GB arena limit and bounded convolution
workspace. The mobile detector screens proxy frames in batches at the 0.3 box score and the
server detector inspects single full-resolution frames at 0.5. A screening batch holds images of one size, runs as one predictor
call and returns one region list per image in input order; region coordinates are in the input
image's own pixels.

## Boundaries

- Depends on: `ort`, `oar-ocr`, `image`, `job_model` and `tracing`.
- Used by: the visual stages in `stages::onscreen_text`; the on-screen text steps in
  `crates/pipeline/src/tasks/onscreen.rs`, which open the detectors and the reader in their
  workers; and `tools/visual_validation/`.
- Rules: an unavailable CUDA provider is an error; no exported model is converted.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md) — runtime selection.

# Local text recognition

PP-OCRv5 detection and recognition and a manga-ocr second reading, from the pinned model files,
in isolated ONNX workers: the detector pool that screens and confirms full-resolution frames, the
stock detectors and the reader.

## Contents

```text
crates/inference/src/ocr/
├── detector_pool/  `DetectorPool`: screening and confirming sessions on CUDA or TensorRT, one thread each
├── manga.rs        bounded encoder/decoder beam search
├── mod.rs          `TextDetection`, the two stock detectors (`OcrDetector`), `OcrReader`, strict CUDA setup
└── pool/           the screening pool's contract: padded frame batches, regions and confirmations
```

## How it works

`detector_pool/` implements the `pool/` contract on our own ONNX Runtime sessions: the mobile
PP-OCRv5 detector screens padded full-resolution frames in batches of a fixed shape on two
threads, and once screening ends the server detector confirms single frames, each on the CUDA
provider or on TensorRT before CUDA. The stock `OcrDetector` runs the same two models through
oar-ocr's predictors on frames of any size, for the tools; the server recognizer reads the crops.
Vertical and uncertain crops receive a second manga-ocr reading. Disagreement remains uncertain.
`read_primary` returns the server recognizer's reading alone, Japanese and Latin alike, for the
read-back check of lettered English, which the Japanese-only manga-ocr must not read. All files
come from the pinned model store. The decoder has bounded beam width and sequence length.

Every detector keeps boxes at the 0.45 box score when screening and 0.5 when confirming. The
stock predictors and the reader run on the process's strict CUDA environment, with heuristic
convolution selection, a 5 GiB arena limit and bounded convolution workspace; a stock screening
batch holds images of one size, runs as one predictor call and returns one region list per image
in input order, in the input image's own pixels. The pool's sessions set their own providers and
options instead.

## Public surface

- `DetectorPool`, `PoolOptions`, `SearchMode` and the `pool` contract (`TextScreening`,
  `ScreenJob`, `ConfirmJob`, `PaddedFrame`, `ScreenShape`, `EngineIdentity`); for the detection
  scan in `crates/stages/src/onscreen_text/detect/`, the `text_detect` step in `crates/pipeline/`
  and `tools/visual_validation/`.
- `TextDetection`, `OcrDetector`, `OcrReader`, `MangaReader` and `OcrError`; for the visual
  stages in `crates/stages/src/onscreen_text/`, `crates/pipeline/` and `tools/visual_validation/`.

## Boundaries

- Depends on: `ort`, `oar-ocr`, `ndarray`, `rayon`, `sha2`, `image`, `job_model`, `tracing` and
  `crate::model_store`.
- Used by: the visual stages in `stages::onscreen_text`; the on-screen text steps in
  `crates/pipeline/src/tasks/onscreen.rs`, which open the detectors and the reader in their
  workers; and `tools/visual_validation/`.
- Rules: an unavailable CUDA or TensorRT provider is an error, never a fall back to another
  provider or the CPU; the models are the pinned ONNX files, and TensorRT engines are built from
  them inside ONNX Runtime and cached per key.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md) — runtime selection.
- [On-screen detection decisions](/documentation/decisions/onscreen_detection.md) — the
  screening and confirmation design.

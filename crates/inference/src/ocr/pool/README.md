# Detector pool contract

What the detection scan hands the detector sessions and what comes back: padded full-resolution
frames in batches, their regions, and single frames confirmed by the server detector.

## Contents

```text
crates/inference/src/ocr/pool/
├── mod.rs  `ScreenShape`, `PaddedFrame`, `ScreenJob`, `ScreenResult`, `ConfirmJob`, `TextScreening`
└── tests/  padding heights, priority order and the initial shape
```

## How it works

A `PaddedFrame` is one frame converted to rgb24 and padded below with black rows to a multiple of
32, the shape the detectors take without stretching. The scan sends `ScreenJob`s of at most
`ScreenShape::batch` frames, each with a sequence number and a `Priority`: bisection probes run
before screening batches that wait. Results come back as `ScreenResult`s in whatever order the
sessions finish, and the scan applies them in its own order. `confirm` runs the server detector on
one frame per occurrence, spread over the sessions, and answers in the order asked.
`ScreenShape::INITIAL`, `SCREEN_SESSIONS` and `CONFIRM_POOL_MIB` are the starting values the
host's pool-by-batch sweep replaces. `EngineIdentity` names the card, driver and TensorRT build a
cached engine was made for; the caller reads it from the driver.

## Boundaries

- Depends on: `job_model` for `Quad`, and the `ocr` module's `OcrError`.
- Used by: the detection scan in `crates/stages/src/onscreen_text/detect/`.
- Rules: every frame of a job has one size; regions are in frame pixels, clipped above the
  padding; confirmations answer in the order asked.

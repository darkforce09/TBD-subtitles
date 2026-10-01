# Detector pool

The PP-OCRv5 detector sessions the detection scan screens and confirms with: our own ONNX
Runtime sessions with a fixed padded input, one thread each, on the CUDA provider or on TensorRT
before CUDA, behind the `TextScreening` trait of `crates/inference/src/ocr/pool/`.

## Contents

```text
crates/inference/src/ocr/detector_pool/
├── batch.rs     the fixed input shape, frame checks and BGR ImageNet normalisation into staging
├── in_order.rs  `InOrder`: screening results released in sequence order
├── mod.rs       `DetectorPool`, `PoolOptions`, `SearchMode` and the `TextScreening` implementation
├── queue.rs     the shared task queue: probes first, then screening, then confirmations
├── regions.rs   DB post-processing per image on rayon, boxes clipped to the frame
├── session/     one session: its spec, providers, TensorRT cache key, warm-up and reopen
├── tests/       fake sessions: padding, clipping, order, priority, phases, reopen, failures
└── worker.rs    the body of one session thread
```

## How it works

```text
scan ──submit(ScreenJob)──▶ queue (Probe before Screen, then by seq)
                                 │ next task
       ┌─────────────────────────┴─────────────────────────┐
  thread 1: screen session                          thread 2: screen session
  normalise frames (+ black slots) → staging        (the same)
  run → probability maps, read in place
  regions per image (rayon) → ScreenResult{seq} ──▶ recv() in finishing order ──▶ InOrder
scan ──confirm(jobs)──▶ every thread closes its screen session
                        then opens a confirm session on its first confirmation
                        ──▶ results gathered back into the order given
```

`DetectorPool::open` validates the options, starts one thread per session and returns once every
screening session has opened and warmed up; a failure on any thread fails the open. Opening is
serialised by a shared lock, so warm-ups never overlap and a second TensorRT session reuses the
engine the first one built. Every frame has the size given in `PoolOptions`; `batch.rs` pads it to
`[batch, 3, height, width]` with height and width rounded up to multiples of 32, fills unused batch
slots with black frames, and writes the values oar-ocr's DB normalisation gives (scale 1/255,
ImageNet mean and standard deviation, BGR, CHW) straight into the session's pinned staging buffer,
one rayon task per row. `regions.rs` copies each image's map once into the array
`DBPostProcess::apply` takes, keeps boxes at 0.3 when screening and 0.5 when confirming, and
clips every corner into the frame, which drops boxes found only in the padding.

`confirm` refuses to start while screening results are still due; it then closes every screening
session before any confirming session opens (`queue.rs` holds the barrier), so peak GPU memory is
the larger of the two phases, never their sum. Only the first `confirm_sessions` threads
(`CONFIRM_SESSIONS`, one) take confirmations; the others wait for the pool to close. A confirming
session opens on its thread's first confirmation, with batch 1 and `confirm_pool_mib`
(`CONFIRM_POOL_MIB`). Every task taken from the queue gets one
answer, an error included, so neither `recv` nor `confirm` waits forever.

`notes()` names the engine, the search mode, the session count and both input shapes, and, per
thread, whether each session runs with the CUDA graph and NHWC or refused them, and for TensorRT
whether the engine was built or reused and where. `warmup_s()` sums the warm-ups;
`engine_build_s()` sums the opening and warm-up of sessions that built a TensorRT engine.

## Public surface

- `DetectorPool::open(models_root, PoolOptions)`: the pool, as a `TextScreening`; for the
  `text_detect` step in `crates/pipeline/` and the detection bench in `tools/visual_validation/`.
- `PoolOptions` (`new(engine, identity, width, height)` for the production values) and
  `SearchMode`; `DEFAULT_VRAM_CAP_MIB`; `TENSORRT_FOLDER`, the cache folder's name under the
  app's data folder.
- `InOrder`: a reorder buffer for screening results; for the detection scan in
  `crates/stages/src/onscreen_text/detect/`.

## Boundaries

- Depends on: `ort` (sessions, providers, `IoBinding`, pinned tensors), `oar-ocr`
  (`DBPostProcess`), `ndarray`, `rayon`, `sha2`, `job_model` (`Quad`, `DetectorEngine`), the
  `ocr` module (`OcrError`, the scores, the strict CUDA environment, the `pool` contract) and
  `crate::model_store::app_data_dir`.
- Used by: the `ocr` module's re-exports; the screening callers listed under Public surface.
- Rules:
  - The input shape is fixed per session: short jobs are padded with black frames and frames of
    another size are refused (`tests/pool.rs`, `tests/batch.rs`).
  - Normalisation matches oar-ocr's DB normalisation (`tests/batch.rs`).
  - Regions are in frame pixels and never reach into the padding (`tests/regions.rs`).
  - Probes run before waiting screening jobs; confirmations answer in the order given, after every
    screening session closed (`tests/queue.rs`, `tests/pool.rs`).
  - A session that cannot open, even without the CUDA graph and NHWC, fails the pool; nothing
    falls back to another provider (`tests/pool.rs`).

## Related documentation

- [On-screen detection decisions](/documentation/decisions/onscreen_detection.md) — the screening
  and confirmation design these sessions run.
- [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md) — the feature the
  detection scan serves.

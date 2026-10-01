# Detector pool benchmark

The `detect-bench` sections that run the production detector pool
(`inference::ocr::DetectorPool`): the CUDA pool-by-batch sweep, one session against two, the fast
against the deterministic search, TensorRT FP16 and FP32, and confirmation on stills.

## Contents

```text
tools/visual_validation/src/detect_bench/pool_runs/
├── compare.rs   boxes of one run against a reference run: exact equality per frame and IoU matches
├── measure.rs   one pool configuration opened, screening or confirming, timed and sampled
├── mod.rs       `Setup`, the pool options of a run, the engine identity, padded frames and jobs
├── plan.rs      `PoolRun`, the sweep grid and the shape the later sections run at
├── rows.rs      `Measured` and its table row, with the graph, NHWC and engine cache read from notes
├── sections.rs  sections 4 to 8 in order, and the CUDA reference run
└── tests/       frames, jobs, options, identity, grid, shape choice, rows and box comparison
```

## How it works

`detect_bench::run` decodes the clip's sample frames as yuv420p at the source's size and converts
each, before any timing, with `media_io::yuv::to_rgb_padded_into` in the stream's own matrix and
range into a `PaddedFrame` (rgb24, black rows below to a multiple of 32), as the scan does.

```text
padded frames ─▶ sweep (CUDA, 2 sessions, every batch × pool) ─▶ chosen shape
               ─▶ one session, two sessions ─────────────────────▶ reference regions
               ─▶ fast ×2, deterministic ×2
               ─▶ TensorRT FP16 then FP32 over the sweep ◀── compared with the reference
first 40 frames ─▶ confirmation: CUDA, TensorRT FP16, FP32 ◀── compared with CUDA
```

`measure.rs` builds every `ScreenJob` (numbered, `Priority::Screen`, the last one short) before
opening the pool, starts the pipeline's `gpu_monitor::Monitor` around opening and screening, opens
`DetectorPool` with `PoolOptions` from `Setup::options`, then submits every job and receives every
result while the bench's `usage::Sampler` measures wall time and GPU busy %. Results are put back
in job order. Warm-up and engine build seconds come from the pool; `rows.rs` reads from
`pool.notes()` whether each session kept the CUDA graph and NHWC (`accepted ×2`, or the refusal)
and whether each TensorRT engine was built or reused from the cache folder. Confirmation opens a
pool, confirms one still per session untimed so every confirm session opens (and builds its
engine), then starts the monitor and times the rest; its warm-up and build seconds are what the
confirm sessions added.

The chosen shape is `--shape-batch` and `--shape-pool-mib` when given, else the sweep's fastest
row, else `ScreenShape::INITIAL`. The reference run is two CUDA sessions at that shape with the
fast search: the two-session row of section 5, else the first fast run of section 6, else a run of
its own when TensorRT or the box images need it. `compare.rs` reports exact equality frame by
frame (corners and scores) and pairs boxes one to one at a bounding-rectangle IoU of at least 0.5.
TensorRT engines are keyed by batch and precision, not pool, so each batch's engine is built at
its first pool size and reused at the others; a second bench run over the same cache folder
measures cached engines only.

## Boundaries

- Depends on: `crates/inference` (`ocr::DetectorPool`, `PoolOptions`, `SearchMode`, the `pool`
  contract, `model_store::manifest::TENSORRT_VERSION`), `crates/media_io` (`yuv`),
  `crates/pipeline` (`measure::gpu_monitor`), `crates/job_model` (`Quad`, `DetectorEngine`), and
  the bench's `table.rs` and `usage.rs`.
- Used by: `tools/visual_validation/src/detect_bench/mod.rs` (`run`), and its `overlay.rs`
  (the reference regions).
- Rules:
  - A failed configuration gives its row with the error in the last cell, and the next row runs
    (`a_failed_row_keeps_its_configuration_and_ends_with_the_error`).
  - The grid holds each batch and pool once, batch first, in the order given
    (`the_default_sweep_is_three_batches_by_four_pools_batch_first`).
  - Frames are padded with black rows to a multiple of 32
    (`a_picture_converts_into_a_frame_padded_with_black_rows`).
  - A run compared with itself is identical with every box matched
    (`a_run_compared_with_itself_is_identical_with_every_box_matched`).

## Related documentation

- [On-screen detection decisions](/documentation/decisions/onscreen_detection.md) — the
  screening shape, sessions and search mode these sections measure.

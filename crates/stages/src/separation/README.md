# Vocal separation stage

The vocal separation stage: the mix split into a vocal [stem](/documentation/glossary.md#stem) and a
background stem, written as 16 kHz mono raw `.f32` files for the stages after it.

## Contents

```text
crates/stages/src/separation/
├── mod.rs       `separate`: FFmpeg at 44.1 kHz stereo ─▶ model overlap-add ─▶ two 16 kHz mono stems
├── resample.rs  a streaming windowed-sinc resampler from 44.1 kHz to 16 kHz
└── tests/       unit tests for the resampler: length, passband level and phase, alias rejection
```

## How it works

`separate` opens a 44.1 kHz stereo `PcmStream`, feeds each one-second chunk to an
`OverlapAdd` around the given model, and turns each run of finished frames into two mono
signals: the vocals, and the mix minus the vocals. Each goes through its own `Resampler` into an
`F32FileWriter`, so memory holds a few model windows at most. The summary records the frames,
the time spent waiting on FFmpeg and the time spent separating; the model comes back to the
caller for its own counters. After each chunk the request's progress callback hears the seconds
separated of `duration_s`.

## Boundaries

- Depends on: `media_io::pcm_stream` (FFmpeg audio and the stem files),
  `inference::onnx::separation` (the driver and the models).
- Used by: `crates/pipeline/src/tasks/media.rs` (the separation step) and `tools/stack_spike/`
  (the separation items).
- Rules:
  - the stage runs in a worker process of its own
    (`gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary` in
    `crates/pipeline/src/graph/tests/graph.rs`);
  - both stems are whole or absent (each moves into place from its `.part` file in
    `F32FileWriter::finish`);
  - the resampler passes speech frequencies and removes content above 8 kHz
    (`a_passband_tone_keeps_its_level_and_phase`, `a_tone_above_eight_kilohertz_is_removed`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#2-vocal-separation) — the models and what
  reads each stem.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#3-vocal-separation) — the separation
  options.

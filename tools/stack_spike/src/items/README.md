# Stack items

The pieces of the ML stack the spike measures, one module each, run inside a worker process.
Each returns the audio seconds it processed, its load and processing times, and its notes.

## Contents

```text
tools/stack_spike/src/items/
├── decode.rs  ffprobe, 16 kHz mono decoded to `mix_16k.f32`, and a 44.1 kHz stereo pass
├── mod.rs     the `Item` list in run order, which items need the GPU, and the `Outcome` they return
└── shots.rs   the scdet scan on the CPU with cut counts per score, and the NVDEC scan time
```

## Boundaries

- Depends on: `media_io` (probe, PCM stream, shot scan); `crate::context::Context` for paths.
- Used by: `tools/stack_spike/src/measure/` (`Item::run` in the worker, `needs_gpu` in the parent).
- Rules: an item reads only the video and the work folder, and writes only the work folder (the
  header in `crates/media_io/src/lib.rs` for the video; review for the rest).

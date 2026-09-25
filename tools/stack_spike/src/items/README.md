# Stack items

The pieces of the ML stack the spike measures, one module each, run inside a worker process.
Each returns the audio seconds it processed, its load and processing times, and its notes.

## Contents

```text
tools/stack_spike/src/items/
├── decode.rs    ffprobe, 16 kHz mono decoded to `mix_16k.f32`, and a 44.1 kHz stereo pass
├── mod.rs       the `Item` list in run order, which items need the GPU, and the `Outcome` they return
├── separate.rs  MDX-Net Voc_FT and Mel-Band RoFormer over the track: stems, levels, WAV excerpts
├── shots.rs     the scdet scan on the CPU with cut counts per score, and the NVDEC scan time
└── vad.rs       earshot and the chunk plan on the mix and both vocal stems, with speech shares
```

## Boundaries

- Depends on: `media_io` (probe, PCM stream, shot scan), `stages::separation`, `stages::vad`,
  `inference::onnx::separation`; `crate::context::Context` for paths and model files.
- Used by: `tools/stack_spike/src/measure/` (`Item::run` in the worker, `needs_gpu` in the parent).
- Rules: an item reads only the video and the work folder, and writes only the work folder (the
  header in `crates/media_io/src/lib.rs` for the video; review for the rest).

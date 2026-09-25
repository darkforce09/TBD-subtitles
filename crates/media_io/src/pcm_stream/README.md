# PCM audio stream

FFmpeg decoding a video's audio to 16 kHz mono or 44.1 kHz stereo 32-bit float through a pipe,
read in fixed-size chunks through a bounded channel so a whole track is never held in memory. The
module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/media_io/src/pcm_stream/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/media_io/src/lib.rs` declares it as a public module.
- Rules: audio is read in fixed-size chunks through a bounded channel and never held whole at
  44.1 kHz (the crate header in `crates/media_io/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — the two audio streams
  and who reads them.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#9-ffmpeg-from-rust) — the FFmpeg command
  and the memory a whole track would take.

# Media input source

The `media_io` library: one module folder for each thing the pipeline reads from a video, which
are the probe result, the decoded audio and the shot-change times. The modules are not written
yet.

## Contents

```text
crates/media_io/src/
├── lib.rs         the crate root: the module list and the crate header
├── pcm_stream/    FFmpeg decoding audio to 32-bit float PCM, read in fixed-size chunks
├── probe/         ffprobe's JSON for a video, and the choice of the English audio track
└── shot_changes/  FFmpeg's `scdet` scan: the times of the shot changes that cue timing snaps to
```

## How it works

The three modules serve the probe and decode stage: `probe` is for the streams, the frame rate
and the English track; `pcm_stream` for the audio at 16 kHz mono and 44.1 kHz stereo; and
`shot_changes` for a second FFmpeg process that scans the cuts. Each module holds only a `mod.rs`
with its header; none of them runs a program yet.

## Public surface

- `pcm_stream`, `probe` and `shot_changes`: public modules with no items yet, for the
  `probe_decode` stage in `crates/stages/src/probe_decode/`.

## Boundaries

- Depends on: nothing yet; the crate declares `child_process` and `job_model` for these modules.
- Used by: nothing yet; `crates/stages/` declares the crate as a dependency.
- Rules: no module holds a whole track in memory, and no module writes to the source video (the
  crate header in `lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — what the probe stage
  reads and writes.

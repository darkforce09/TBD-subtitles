# Media input

The `media_io` crate: everything the pipeline reads from a video file, which is the probe result,
the audio as 32-bit float PCM, the [shot-change](/documentation/glossary.md#shot-change) times and
the video frames, and the one video it writes, the localized video, through FFmpeg and ffprobe run
as child processes, with no libav linked.

## Contents

```text
crates/media_io/
├── Cargo.toml  the `media_io` library package: `child_process`, `job_model`, and serde for ffprobe JSON
└── src/        the ffprobe probe, the PCM audio stream, the shot-change scan, the frames, the encode
```

## How it works

The crate is split by what it reads from the video: `probe` is for ffprobe's JSON and the choice
of the English audio track, `pcm_stream` for FFmpeg decoding the audio through a pipe in
fixed-size chunks, `shot_changes` for FFmpeg's `scdet` scan of a small scaled copy of the
video, `video_frames` for frames paired with presentation times read from the packet table
before decoding (scaled RGB, native raw frames, stills and region crops), `encode` for FFmpeg
encoding raw frames from a pipe into Matroska with the source's audio copied, and `preview` for
the FFmpeg command lines the app's review views run to play a clip. The crate header places every
FFmpeg and ffprobe run behind `child_process`, whose drain threads keep FFmpeg's stderr from
blocking the audio pipe, and returns `job_model` types. `src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p media_io   # the library
cargo test -p media_io    # 76 unit tests, 8 ignored (need FFmpeg); many run FFmpeg on generated media
```

The app runs FFmpeg 8.1 on the host; inside the development container, run anything that calls
FFmpeg through `distrobox-host-exec`, as the development environment runbook says.

## Configuration

None: the crate reads no setting.

## Public surface

- The library `media_io`: `Programs`, `MediaError`, and the public modules `encode`,
  `pcm_stream`, `preview`, `probe`, `shot_changes` and `video_frames`.
- No binary.

## Boundaries

- Depends on: `child_process` (FFmpeg and ffprobe with deadlines), `job_model` (the output
  types), `serde` and `serde_json`; the programs `ffmpeg` and `ffprobe`. No libav crate.
- Used by: `crates/stages/`, `crates/pipeline/`, `tools/stack_spike/`, `tools/stack_spike_ggml/`,
  `tools/visual_validation/`, and the app (`Programs` for the system check, `preview` for the
  line and text review players).
- Rules:
  - the crate sits in layer 1 and depends only on layer 0 crates (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - audio is streamed in fixed-size chunks through a bounded channel and never held whole at
    44.1 kHz, and the source video is only read (the crate header in `crates/media_io/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — probing, decoding and
  the shot-change scan.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#9-ffmpeg-from-rust) — the FFmpeg
  streaming pattern and its memory figures.
- [Development environment](/documentation/runbooks/development_environment.md#host-and-container)
  — which FFmpeg runs where.

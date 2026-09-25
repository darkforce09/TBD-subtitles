# Media input

The `media_io` crate: everything the pipeline reads from a video file, which is the probe result,
the audio as 32-bit float PCM and the [shot-change](/documentation/glossary.md#shot-change) times,
through FFmpeg and ffprobe run as child processes, with no libav linked. Its three modules are not
written yet.

## Contents

```text
crates/media_io/
├── Cargo.toml  the `media_io` library package; depends on `child_process` and `job_model`
└── src/        the ffprobe probe, the PCM audio stream and the shot-change scan
```

## How it works

The crate is split by what it reads from the video: `probe` is for ffprobe's JSON and the choice
of the English audio track, `pcm_stream` for FFmpeg decoding the audio through a pipe in
fixed-size chunks, and `shot_changes` for FFmpeg's `scdet` scan of a small scaled copy of the
video. The crate header places every FFmpeg and ffprobe run behind `child_process`, whose drain
threads keep FFmpeg's stderr from blocking the audio pipe, and returns `job_model` types. Each
module holds only its one-line header; no code runs FFmpeg yet. `src/README.md` describes each
module.

## Getting started

Run these from the repository root:

```bash
cargo build -p media_io   # the module declarations; nothing runs FFmpeg yet
cargo test -p media_io    # runs 0 tests: no module holds code yet
```

The app runs FFmpeg 8.1 on the host; inside the development container, run anything that calls
FFmpeg through `distrobox-host-exec`, as the development environment runbook says.

## Configuration

None: the crate reads no setting.

## Public surface

- The library `media_io`, with the public modules `pcm_stream`, `probe` and `shot_changes`; they
  hold no items yet.
- No binary.

## Boundaries

- Depends on: `child_process` and `job_model`, declared in `Cargo.toml` and not called yet. No
  libav crate.
- Used by: `crates/stages/`, which declares it as a dependency.
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

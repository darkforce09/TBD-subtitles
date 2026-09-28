# Media input source

The `media_io` library: one module folder for each thing the pipeline reads from a video, which
are the probe result, the decoded audio and the shot-change times, and the crate's `Programs`
and `MediaError`.

## Contents

```text
crates/media_io/src/
├── lib.rs         the crate root: the programs to run, the error type and the module list
├── pcm_stream/    FFmpeg decoding audio to 32-bit float PCM, read in fixed-size chunks
├── preview/       FFmpeg command lines for a clip: its sound with a silence pad, and its frames
├── probe/         ffprobe's JSON for a video, and the choice of the English audio track
├── shot_changes/  FFmpeg's `scdet` scan: the times of the shot changes that cue timing snaps to
└── tests/         `lib.rs`'s own tests: `Programs::beside` picking the bundled pair or falling back
```

## How it works

The three modules serve the probe and decode stage: `probe` is for the streams, the frame rate
and the English track; `pcm_stream` for the audio at 16 kHz mono and 44.1 kHz stereo; and
`shot_changes` for a second FFmpeg process that scans the cuts. `Programs` names the `ffmpeg` and
`ffprobe` to run: the bare names on the `PATH` by default, or the pair bundled at
`<exe_dir>/ffmpeg/` when `Programs::beside`/`beside_current_exe` finds both there (`bundled` says
which). Every failure is a `MediaError`: the program could not run, exited non-zero, printed
something unreadable, or the video has no usable audio track.

## Public surface

- `probe::{probe, parse, english_track}`, `pcm_stream::{PcmStream, PcmRequest, PcmFormat,
  write_f32_file, F32FileReader, F32FileWriter}`, `shot_changes::{scan, parse}`,
  `Programs` (with `beside` and `beside_current_exe`) and `MediaError`: for `crates/stages/`,
  `crates/pipeline/`, `apps/tbd_subtitles/`, `tools/stack_spike/` and the `probe_decode` stage.

## Boundaries

- Depends on: `child_process` for FFmpeg and ffprobe, `job_model` for the output types,
  `serde_json` for ffprobe's JSON.
- Used by: `tools/stack_spike/`; `crates/stages/` declares the crate as a dependency.
- Rules: no module holds a whole track in memory, and no module writes to the source video (the
  crate header in `lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — what the probe stage
  reads and writes.

# Subtitle formats

The `subtitle_formats` crate: the [cue](/documentation/glossary.md#cue) model, one writer per
subtitle format (SRT, WebVTT and [ASS](/documentation/glossary.md#ass)), and import of existing
subtitle files. The cue model and the SRT writer hold code; the WebVTT and ASS writers and import
are not written yet.

## Contents

```text
crates/subtitle_formats/
├── Cargo.toml  the `subtitle_formats` library package: `job_model`, `serde`; `serde_json` in tests
└── src/        the cue model, the format writers and subtitle import
```

## How it works

`cue` holds the one cue model the cue stage builds, the quality check reads and the writers turn
into text: a `CueTrack` is the video's `FrameRate` and its cues in time order, and each `Cue` runs
from frame `start` up to, not including, frame `end`, with one or two `CueLine`s (plain or italic)
and a `CueKind` (dialogue, sound or music). Times are whole frames, so nothing downstream can place
a cue between frames; `FrameRate` converts frames to seconds and milliseconds and seconds back to
frames. The track is stored as `cues.json` in the job's work directory.

`writers::srt` turns a track into SubRip text, rounding each frame to the nearest millisecond.
`writers::vtt`, `writers::ass` and `import` hold only their headers. `src/README.md` describes
each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p subtitle_formats   # the library
cargo test -p subtitle_formats    # 11 unit tests for the cue model and SRT, well under a second
```

## Configuration

None: the crate reads no setting.

## Public surface

- `cue::{FrameRate, CueKind, CueLine, Cue, CueTrack}`: the cue model, for the cue and quality
  check stages in `crates/stages/src/cues/` and `crates/stages/src/qc/`, and for the cue step in
  `crates/pipeline/src/tasks/layout.rs`.
- `writers::srt::{write, timestamp}`: the SRT file, for the output step in
  `crates/pipeline/src/tasks/layout.rs`.
- `writers::vtt`, `writers::ass` and `import`: public modules with no items yet.
- No binary.

## Boundaries

- Depends on: `serde` with derive, for the cue model's JSON; `job_model`, declared in `Cargo.toml`
  and not called yet; `serde_json` in the tests only.
- Used by: `crates/stages/` (`cues` and `qc`) and `crates/pipeline/` (`tasks/layout.rs`).
- Rules:
  - the crate sits in layer 1 and depends only on layer 0 crates (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - a frame rate never has a zero part, and frames convert to milliseconds by rounding
    (`a_zero_rate_is_refused`, `film_frames_round_to_milliseconds`,
    `ntsc_frames_round_to_milliseconds` in `crates/subtitle_formats/src/cue/tests/cue.rs`);
  - every file is UTF-8, and a writer never changes a cue's times or text (the crate header in
    `crates/subtitle_formats/src/lib.rs`; `the_writer_keeps_text_as_given` in
    `crates/subtitle_formats/src/writers/srt/tests/srt.rs`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — the
  formats and the layout they carry.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#10-subtitle-files) — why the writers are
  our own and crates serve only import.

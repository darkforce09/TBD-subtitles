# Subtitle formats

The `subtitle_formats` crate: the [cue](/documentation/glossary.md#cue) model, one writer per
subtitle format (SRT, WebVTT and [ASS](/documentation/glossary.md#ass)), and import of existing
subtitle files. The cue model and the three writers hold code; import is not written yet.

## Contents

```text
crates/subtitle_formats/
├── Cargo.toml  the library package: `job_model`, `serde`, `rkyv`; `serde_json` in tests
└── src/        the cue model, the format writers and subtitle import
```

## How it works

`cue` holds the one cue model the cue stage builds, the quality check reads and the writers turn
into text: a `CueTrack` is the video's `FrameRate` and its cues in time order, and each `Cue` runs
from frame `start` up to, not including, frame `end`, with one or two `CueLine`s (plain or italic)
and a `CueKind` (dialogue, sound or music). Times are whole frames, so nothing downstream can place
a cue between frames; `FrameRate` converts frames to seconds and milliseconds and seconds back to
frames. The track archives with rkyv and is stored as `outputs/cues` in the job's database
(`tbd-subtitles dump` prints it).

`writers::srt` and `writers::vtt` turn a track into SubRip and WebVTT text, rounding each frame to
the nearest millisecond; `writers::ass` writes an ASS script in centisecond times, and its
`write_with` moves each cue that would cover English lettered into the picture to the top, for
the localized video's subtitle file. `import` holds only its header. `src/README.md` describes
each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p subtitle_formats   # the library
cargo test -p subtitle_formats    # 25 unit tests of the cue model and writers, under a second
```

## Configuration

None: the crate reads no setting.

## Public surface

- `cue::{FrameRate, CueKind, CueLine, Cue, CueTrack}`: the cue model, for the cue, on-screen
  text and quality check stages in `crates/stages/src/cues/`, `crates/stages/src/onscreen_text/`
  and `crates/stages/src/qc/`, for the cue and output steps in
  `crates/pipeline/src/tasks/layout.rs` and `crates/pipeline/src/tasks/onscreen.rs`, and for the
  record kinds of the job store in `crates/pipeline/src/work_dir/store/kinds.rs`.
- `writers::srt::{write, timestamp}`, `writers::vtt::{write, timestamp}` and
  `writers::ass::{write, write_with, timestamp, Obstacle, PLAY_RES}`: the three files, for the
  output step in `crates/pipeline/src/tasks/layout.rs`; `writers::ass::write` also for
  `tools/visual_validation/src/pilot.rs`.
- `import`: a public module with no items yet.
- No binary.

## Boundaries

- Depends on: `serde` with derive, for the cue model's JSON; `rkyv`, for its archive in the job
  database, the format named in full as in `job_model`; `job_model`, declared in `Cargo.toml`
  and not called yet; `serde_json` in the tests only.
- Used by: `crates/stages/` (`cues`, `onscreen_text` and `qc`), `crates/pipeline/`
  (`tasks/layout.rs`, `tasks/onscreen.rs` and `work_dir/store/kinds.rs`) and
  `tools/visual_validation/` (`src/pilot.rs`).
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

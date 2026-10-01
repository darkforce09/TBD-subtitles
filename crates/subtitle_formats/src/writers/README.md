# Subtitle writers

One writer per subtitle format, each turning a cue track into a UTF-8 file: SubRip, WebVTT and
Advanced SubStation Alpha.

## Contents

```text
crates/subtitle_formats/src/writers/
├── ass/    Advanced SubStation Alpha: a script header, a bottom-centred style, centisecond times
├── mod.rs  the module header and the three format declarations
├── srt/    SubRip: numbered cues, comma-separated millisecond times, italic tags
└── vtt/    WebVTT: the header line, dot-separated millisecond times, italic tags
```

## How it works

Each format module is for one file type and returns the whole file as a string. `srt/` writes the
default output; `vtt/` and `ass/` write the other two formats the owner can choose. `mod.rs`
declares the three.

## Public surface

- `writers::srt::{write, timestamp}`, `writers::vtt::{write, timestamp}` and
  `writers::ass::{write, timestamp}`: the three files, for the output step in
  `crates/pipeline/src/tasks/layout.rs`.
- `writers::ass::{write_with, Obstacle, PLAY_RES}`: the ASS file with each cue that would cover
  writing in the picture moved to the top, for the localized video's subtitle file.

## Boundaries

- Depends on: `crate::cue` (`CueTrack`, `FrameRate`).
- Used by: `crates/pipeline/src/tasks/layout.rs`, through the writer of the job's output format;
  `tools/visual_validation/src/pilot.rs`, through `ass::write` for its preview script.
- Rules: every writer produces UTF-8 and never changes a cue's times or text (the header in
  `mod.rs` and the crate header in `crates/subtitle_formats/src/lib.rs`;
  `the_writer_keeps_text_as_given` in `crates/subtitle_formats/src/writers/srt/tests/srt.rs`); the
  WebVTT and ASS writers escape what their format reserves so the text shows as given.

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — which
  format is written when.

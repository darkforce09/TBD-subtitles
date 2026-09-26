# Subtitle writers

One writer per subtitle format, each turning a cue track into a UTF-8 file: SubRip, WebVTT and
Advanced SubStation Alpha. The SubRip writer holds code; the WebVTT and ASS writers are not written
yet.

## Contents

```text
crates/subtitle_formats/src/writers/
├── ass/    Advanced SubStation Alpha: styles, top placement and exact positions for sign subtitles
├── mod.rs  the module header and the three format declarations
├── srt/    SubRip: numbered cues, comma-separated millisecond times, italic tags
└── vtt/    WebVTT: the header line, dot-separated millisecond times, italic tags
```

## How it works

Each format module is for one file type and returns the whole file as a string. `srt/` writes the
default output; `ass/` is for when a positioned [sign](/documentation/glossary.md#sign) subtitle
needs `{\an8}` or `\pos` placement; `vtt/` is for WebVTT. `mod.rs` declares the three; `ass/` and
`vtt/` hold only their headers.

## Public surface

- `writers::srt::{write, timestamp}`: the SRT file, for the output step in
  `crates/pipeline/src/tasks/layout.rs`.
- `writers::ass` and `writers::vtt`: public modules with no items yet.

## Boundaries

- Depends on: `crate::cue` (`CueTrack`, `FrameRate`) in `srt/`.
- Used by: `crates/pipeline/src/tasks/layout.rs`, through `writers::srt`.
- Rules: every writer produces UTF-8 and never changes a cue's times or text (the header in
  `mod.rs` and the crate header in `crates/subtitle_formats/src/lib.rs`;
  `the_writer_keeps_text_as_given` in `crates/subtitle_formats/src/writers/srt/tests/srt.rs`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — which
  format is written when.

# Subtitle formats source

The `subtitle_formats` library: the cue model, the writers for each subtitle format, and the import
of existing subtitle files. The cue model and the SRT writer hold code; the WebVTT and ASS writers
and import are not written yet.

## Contents

```text
crates/subtitle_formats/src/
├── cue/      the cue model: frame rate, cues in whole frames, one or two lines, italics, cue kind
├── import/   reading existing subtitle files, such as reference subtitles
├── lib.rs    the crate root: the module list and the crate header
└── writers/  one writer per subtitle format: SRT, WebVTT and ASS
```

## How it works

The cue stage builds a `cue::CueTrack`; the quality check reads it; the output step hands it to
`writers::srt::write`, which returns the file's text. `import/` is for reading another file back
into the same model, and `writers/` also declares the WebVTT and ASS modules; those three hold
only their headers.

## Public surface

- `cue`: `FrameRate`, `CueKind`, `CueLine`, `Cue` and `CueTrack`, for `crates/stages/src/cues/`,
  `crates/stages/src/qc/` and `crates/pipeline/src/tasks/layout.rs`.
- `writers::srt`: `write` and `timestamp`, for `crates/pipeline/src/tasks/layout.rs`.
- `import`, `writers::vtt` and `writers::ass`: public modules with no items yet.

## Boundaries

- Depends on: `serde` and `rkyv` in `cue/`; the crate also declares `job_model`.
- Used by: `crates/stages/` (`cues` and `qc`) and `crates/pipeline/`.
- Rules: every file is UTF-8, and a writer never changes a cue's times or text (the crate header in
  `lib.rs`); every cue time is a whole frame (the header in `cue/mod.rs`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — what a cue may hold and
  how it is laid out.

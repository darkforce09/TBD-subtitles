# Subtitle formats source

The `subtitle_formats` library: the cue model, the writers for each subtitle format, and the import
of existing subtitle files. The cue model and the three writers hold code; import is not written
yet.

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
the writer of the job's format (`writers::srt::write`, `writers::vtt::write` or
`writers::ass::write`), which returns the file's text, and to `writers::ass::write_with` for the
localized video's subtitle file. `import/` is for reading another file back into the same model;
it holds only its header.

## Public surface

- `cue`: `FrameRate`, `CueKind`, `CueLine`, `Cue` and `CueTrack`, for `crates/stages/src/cues/`,
  `crates/stages/src/onscreen_text/`, `crates/stages/src/qc/`, `crates/pipeline/src/tasks/` and
  the job store's record kinds in `crates/pipeline/src/work_dir/store/kinds.rs`.
- `writers::srt`, `writers::vtt` and `writers::ass`: `write` and `timestamp`, and for ASS
  `write_with`, `Obstacle` and `PLAY_RES`, for `crates/pipeline/src/tasks/layout.rs`;
  `writers::ass::write` also for `tools/visual_validation/src/pilot.rs`.
- `import`: a public module with no items yet.

## Boundaries

- Depends on: `serde` and `rkyv` in `cue/`; the crate also declares `job_model`.
- Used by: `crates/stages/` (`cues`, `onscreen_text` and `qc`), `crates/pipeline/` and
  `tools/visual_validation/`.
- Rules: every file is UTF-8, and a writer never changes a cue's times or text (the crate header in
  `lib.rs`); every cue time is a whole frame (the header in `cue/mod.rs`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — what a cue may hold and
  how it is laid out.

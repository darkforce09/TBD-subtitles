# Subtitle formats source

The `subtitle_formats` library: the cue model, the writers for each subtitle format, and the import
of existing subtitle files. The modules are not written yet.

## Contents

```text
crates/subtitle_formats/src/
├── cue/      the cue model: times, one or two lines, italics, speaker dashes, sound cues, position
├── import/   reading existing subtitle files, such as reference subtitles
├── lib.rs    the crate root: the module list and the crate header
└── writers/  one writer per subtitle format: SRT, WebVTT and ASS
```

## How it works

The cue stage builds values of the `cue` model; the output stage hands them to one of `writers/`,
which returns the file's text; `import/` reads another file back into the same model. Each module
holds only its header, and `writers/` declares its three format modules.

## Public surface

- `cue`, `import` and `writers`: public modules with no items yet, for the cue building and output
  stages in `crates/stages/src/cues/` and `crates/stages/src/output/`.

## Boundaries

- Depends on: nothing yet; the crate declares `job_model` for these modules.
- Used by: nothing yet; `crates/stages/` declares the crate as a dependency.
- Rules: every file is UTF-8, and a writer never changes a cue's times or text (the crate header in
  `lib.rs`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — what a cue may hold and
  how it is laid out.

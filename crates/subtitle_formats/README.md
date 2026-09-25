# Subtitle formats

The `subtitle_formats` crate: the [cue](/documentation/glossary.md#cue) model, one writer per
subtitle format (SRT, WebVTT and [ASS](/documentation/glossary.md#ass)), and import of existing
subtitle files. Its modules are not written yet.

## Contents

```text
crates/subtitle_formats/
├── Cargo.toml  the `subtitle_formats` library package; depends on `job_model`
└── src/        the cue model, the format writers and subtitle import
```

## How it works

`cue` is for the one cue model every stage and writer shares: start, end, one or two lines,
italics, speaker dashes, sound cues and position. `writers` is for turning cues into text, one
module per format, and `import` for reading subtitle files that already exist, such as reference
subtitles whose names and sign translations the pipeline reuses. Each module holds only its header.
`src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p subtitle_formats   # the module declarations
cargo test -p subtitle_formats    # runs 0 tests: no module holds code yet
```

## Configuration

None: the crate reads no setting.

## Public surface

- The library `subtitle_formats`, with the public modules `cue`, `import` and `writers` (holding
  `writers::ass`, `writers::srt` and `writers::vtt`); they hold no items yet.
- No binary.

## Boundaries

- Depends on: `job_model`, declared in `Cargo.toml` and not called yet.
- Used by: `crates/stages/`, which declares it as a dependency.
- Rules:
  - the crate sits in layer 1 and depends only on layer 0 crates (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - every file is UTF-8, and a writer never changes a cue's times or text (the crate header in
    `crates/subtitle_formats/src/lib.rs`; no test holds these yet).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — the
  formats and the layout they carry.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#10-subtitle-files) — why the writers are
  our own and crates serve only import.

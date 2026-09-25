# Cue model

The [cue](/documentation/glossary.md#cue) model: start, end, one or two lines, italics, speaker
dashes, [sound cues](/documentation/glossary.md#sound-cue) and position. The module's code is not
written yet; `mod.rs` holds only its header.

## Contents

```text
crates/subtitle_formats/src/cue/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/subtitle_formats/src/lib.rs` declares it as a public module.
- Rules: none of its own beyond the crate's; see the
  [crate README](/crates/subtitle_formats/README.md#boundaries).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#text-and-layout) — lines,
  speakers, italics and sound cues.

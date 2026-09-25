# Subtitle writers

One writer per subtitle format, each turning cues into a UTF-8 file: SubRip, WebVTT and Advanced
SubStation Alpha. The writers are not written yet.

## Contents

```text
crates/subtitle_formats/src/writers/
├── ass/    Advanced SubStation Alpha: styles, top placement and exact positions for sign subtitles
├── mod.rs  the module header and the three format declarations
├── srt/    SubRip: numbered cues, comma-separated millisecond times, italic tags
└── vtt/    WebVTT: the header line, dot-separated millisecond times, italic tags
```

## How it works

Each format module is for one file type. `srt/` writes the default output; `ass/` is for when a
positioned [sign](/documentation/glossary.md#sign) subtitle needs `{\an8}` or `\pos` placement;
`vtt/` is for WebVTT. `mod.rs` declares the three; each holds only its header.

## Public surface

- `writers::ass`, `writers::srt` and `writers::vtt`: public modules with no items yet, for the
  output stage in `crates/stages/src/output/`.

## Boundaries

- Depends on: nothing yet; the modules hold no code.
- Used by: nothing; `crates/subtitle_formats/src/lib.rs` declares it as a public module.
- Rules: every writer produces UTF-8 and never changes a cue's times or text (the header in
  `mod.rs` and the crate header in `crates/subtitle_formats/src/lib.rs`).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — which
  format is written when.

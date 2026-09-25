# WebVTT writer

WebVTT (`.vtt`): the `WEBVTT` header, `HH:MM:SS.mmm` times and `<i>` italics. The module's code is
not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/subtitle_formats/src/writers/vtt/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/subtitle_formats/src/writers/mod.rs` declares it as a public module.
- Rules: none of their own beyond the writers'; see the
  [writers README](/crates/subtitle_formats/src/writers/README.md#boundaries).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#10-subtitle-files) — the one writer for
  SRT, WebVTT and ASS, and the crates that read subtitle files.

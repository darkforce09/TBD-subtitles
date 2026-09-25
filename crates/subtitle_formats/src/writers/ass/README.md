# ASS writer

[Advanced SubStation Alpha](/documentation/glossary.md#ass) (`.ass`): styles, and `{\an8}` and
`\pos` placement for [sign](/documentation/glossary.md#sign) subtitles. The module's code is not
written yet; `mod.rs` holds only its header.

## Contents

```text
crates/subtitle_formats/src/writers/ass/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/subtitle_formats/src/writers/mod.rs` declares it as a public module.
- Rules: none of their own beyond the writers'; see the
  [writers README](/crates/subtitle_formats/src/writers/README.md#boundaries).

## Related documentation

- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md#output-formats) — when
  ASS is written.

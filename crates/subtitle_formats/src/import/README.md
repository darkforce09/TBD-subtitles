# Subtitle import

Reading existing subtitle files, such as the reference subtitles whose names and
[sign](/documentation/glossary.md#sign) translations the pipeline reuses. The module's code is not
written yet; `mod.rs` holds only its header.

## Contents

```text
crates/subtitle_formats/src/import/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/subtitle_formats/src/lib.rs` declares it as a public module.
- Rules: import reads only the files it is given (the crate header in
  `crates/subtitle_formats/src/lib.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#10-subtitle-files) — the crates that can
  read existing subtitle files.
- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — reference subtitles as
  meaning hints.

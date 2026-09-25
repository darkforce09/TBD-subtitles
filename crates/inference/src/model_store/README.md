# Model store

The models folder: the compiled-in manifest of pinned URLs and checksums, downloads on first use,
and verification of files already present. The module's code is not written yet; `mod.rs` holds
only its header.

## Contents

```text
crates/inference/src/model_store/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/inference/src/lib.rs` declares it as a public module.
- Rules: models are downloaded already exported, from pinned URLs with checksums, and never
  converted (the crate header in `crates/inference/src/lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#models) — the models folder and
  its manifest.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#recommended-stack) — the model files
  per capability.

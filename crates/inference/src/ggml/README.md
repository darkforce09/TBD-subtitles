# Ggml models

Models run through ggml-based crates (whisper-rs, transcribe-cpp, crispasr). Each bundles its own
ggml, so each runs in a [worker process](/documentation/glossary.md#worker-process) of its own.
The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/inference/src/ggml/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/inference/src/lib.rs` declares it as a public module.
- Rules: no two ggml-bundling crates link into one binary (the headers in
  `crates/inference/src/lib.rs` and `mod.rs`; no gate holds it).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#hard-gaps) — why the ggml crates clash.
- [Decisions](/documentation/decisions.md) — native runtimes and one worker process per GPU stage.

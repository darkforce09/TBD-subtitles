# Speech recognition stage

The [speech recognition](/documentation/glossary.md#asr) stage: each speech engine run over the
chunk plan, keeping its words with their times and confidences. The module's code is not written
yet; `mod.rs` holds only its header.

## Contents

```text
crates/stages/src/asr/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/stages/src/lib.rs` declares it as a public module.
- Rules:
  - the stage runs in a worker process of its own (`only_model_stages_run_in_a_worker` in
    `crates/job_model/src/stage/tests/stage_name.rs`);
  - its output is complete or absent, never partial (the crate header in
    `crates/stages/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#4-speech-recognition) — the backbone and the
  second engine.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#1-speech-recognition) — the engines and
  their model files.

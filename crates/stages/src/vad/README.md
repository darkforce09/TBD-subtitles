# Voice activity stage

The [voice activity](/documentation/glossary.md#vad) stage: speech found in the vocal stem, and the
chunk plan every speech engine transcribes. The module's code is not written yet; `mod.rs` holds
only its header.

## Contents

```text
crates/stages/src/vad/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/stages/src/lib.rs` declares it as a public module.
- Rules:
  - the stage runs inside the job runner, not in a worker (`only_model_stages_run_in_a_worker` in
    `crates/job_model/src/stage/tests/stage_name.rs`);
  - its output is complete or absent, never partial (the crate header in
    `crates/stages/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#3-voice-activity-and-chunk-plan) — padding,
  merging and the chunk plan.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#2-voice-activity-detection) — the
  detectors.

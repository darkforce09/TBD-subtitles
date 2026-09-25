# Vocal separation stage

The vocal separation stage: the mix split into a vocal [stem](/documentation/glossary.md#stem) and a
background stem. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/stages/src/separation/
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

- [Pipeline](/documentation/architecture/pipeline.md#2-vocal-separation) — the models and what
  reads each stem.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#3-vocal-separation) — the separation
  options.

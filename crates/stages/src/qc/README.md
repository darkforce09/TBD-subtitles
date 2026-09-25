# Quality check stage

The quality check stage: the cues checked against the layout, timing and coverage rules, and the job
report written. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/stages/src/qc/
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

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — the checks and the report.

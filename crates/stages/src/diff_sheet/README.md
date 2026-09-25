# Diff sheet stage

The [diff sheet](/documentation/glossary.md#diff-sheet) stage: every engine's words aligned to the
[backbone engine](/documentation/glossary.md#backbone-engine)'s, written as the sheet the language
model reads. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/stages/src/diff_sheet/
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

- [Pipeline](/documentation/architecture/pipeline.md#5-diff-sheet) — the sheet's line format,
  locked words and orphans.

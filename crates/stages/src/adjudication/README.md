# Adjudication stage

The [adjudication](/documentation/glossary.md#adjudication) stage: the language model settles each
disagreement in the [diff sheet](/documentation/glossary.md#diff-sheet), chooses the sound cues and
flags doubt, then its answer is checked. The module's code is not written yet; `mod.rs` holds only
its header.

## Contents

```text
crates/stages/src/adjudication/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/stages/src/lib.rs` declares it as a public module.
- Rules:
  - the stage runs in a worker process of its own (`only_model_stages_run_in_a_worker` in
    `crates/job_model/src/stage/tests/stage_name.rs`);
  - its output is complete or absent, never partial (the crate header in
    `crates/stages/src/lib.rs`);
  - it runs after sound events, whose candidates it chooses from, and before alignment
    (`sound_events_come_before_adjudication_which_chooses_the_cues`, same test file).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — the input, the answer, the
  automatic checks and re-decoding.

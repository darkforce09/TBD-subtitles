# Forced alignment stage

The [forced alignment](/documentation/glossary.md#forced-alignment) stage: the final text aligned
against the vocal stem, with fallbacks, recording which source timed each word. The module's code is
not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/stages/src/alignment/
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
  - it runs after adjudication, whose text it times
    (`sound_events_come_before_adjudication_which_chooses_the_cues`, same test file).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#7-forced-alignment) — alignment blocks, the
  pass checks and the fallbacks.

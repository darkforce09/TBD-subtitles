# Sound events stage

The sound events stage: sound events detected on the background and vocal
[stems](/documentation/glossary.md#stem), with window scores turned into candidate
[sound cues](/documentation/glossary.md#sound-cue). The module's code is not written yet; `mod.rs`
holds only its header.

## Contents

```text
crates/stages/src/sound_events/
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
  - it runs before adjudication, which chooses among its candidates
    (`sound_events_come_before_adjudication_which_chooses_the_cues`, same test file).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#8-sound-events) — the classes, windows and
  thresholds.
- [Rust ML stack](/documentation/research/rust_ml_stack.md#5-sound-events) — the models.

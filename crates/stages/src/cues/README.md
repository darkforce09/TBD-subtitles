# Cue building stage

The cue building stage: the aligned words and the chosen sound cues laid out as subtitle
[cues](/documentation/glossary.md#cue), snapped to frames and
[shot changes](/documentation/glossary.md#shot-change). The module's code is not written yet;
`mod.rs` holds only its header.

## Contents

```text
crates/stages/src/cues/
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

- [Pipeline](/documentation/architecture/pipeline.md#9-cue-building) — the layout and timing rules
  in short.
- [Subtitle style rules](/documentation/architecture/subtitle_style_rules.md) — the full rules.

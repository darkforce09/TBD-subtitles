# Output stage

The output stage: the subtitle file written beside the video with the video's base name, backing up
any file it replaces. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/stages/src/output/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/stages/src/lib.rs` declares it as a public module.
- Rules:
  - the stage runs inside the job runner, not in a worker (`only_model_stages_run_in_a_worker` in
    `crates/job_model/src/stage/tests/stage_name.rs`);
  - its output is complete or absent, never partial (the crate header in
    `crates/stages/src/lib.rs`);
  - it is the only stage that writes beside the video rather than into the work directory
    (the crate header in `crates/stages/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#11-output) — the file name, format and backup.
- [Decisions](/documentation/decisions.md) — why subtitles live beside the video.

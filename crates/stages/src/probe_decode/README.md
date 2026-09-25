# Probe and decode stage

The probe and decode stage: ffprobe the video, stream its audio into the work directory and scan its
[shot changes](/documentation/glossary.md#shot-change). The module's code is not written yet;
`mod.rs` holds only its header.

## Contents

```text
crates/stages/src/probe_decode/
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

- [Pipeline](/documentation/architecture/pipeline.md#1-probe-and-decode) — the probe, the two audio
  streams and the shot-change scan.

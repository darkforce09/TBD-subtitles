# Stage outputs

The typed output of each [stage](/documentation/glossary.md#stage), one JSON file per stage in the
job's work directory. The probe result and the shot changes are written; the other stages'
outputs are added as the stages are built.

## Contents

```text
crates/job_model/src/outputs/
├── mod.rs    the module list and the re-exports
├── probe.rs  the probe result: duration, the video stream and its frame rate, the audio tracks
└── shots.rs  the shot changes: every scdet cut with its time and score
```

## Boundaries

- Depends on: `serde` for the derives.
- Used by: `crates/media_io/` (the probe and the shot-change scan return these types) and
  `tools/stack_spike/` (writes them as `probe.json` and `shots.json`).
- Rules: an output type changes only together with every stage that reads or writes it, and its
  JSON names stay stable so a resumed job reads what an earlier run wrote (the crate header in
  `crates/job_model/src/lib.rs`); shot changes keep every scdet score so the cue stage chooses the
  threshold (review).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the file
  each stage writes.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage produces.

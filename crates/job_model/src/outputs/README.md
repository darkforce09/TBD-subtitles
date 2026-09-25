# Stage outputs

The typed output of each [stage](/documentation/glossary.md#stage), one JSON file per stage in the
job's work directory. The probe result, the shot changes, the speech plan and the engine
transcripts are written; the other stages' outputs are added as the stages are built.

## Contents

```text
crates/job_model/src/outputs/
├── mod.rs     the module list and the re-exports
├── probe.rs   the probe result: duration, the video stream and its frame rate, the audio tracks
├── shots.rs   the shot changes: every scdet cut with its time and score
├── speech.rs  the speech plan: speech regions and the chunks the engines transcribe
└── words.rs   an engine's transcript: timed words per chunk, with the engine and its input
```

## Boundaries

- Depends on: `serde` for the derives.
- Used by: `crates/media_io/` (the probe and the shot-change scan), `crates/stages/src/vad/` (the
  speech plan), `crates/stages/src/asr/` and `crates/inference/` (the transcripts), and the stack
  spike tools, which write them as `probe.json`, `shots.json`, `vad.<input>.json` and
  `asr.<engine>.<input>.json`.
- Rules: an output type changes only together with every stage that reads or writes it, and its
  JSON names stay stable so a resumed job reads what an earlier run wrote (the crate header in
  `crates/job_model/src/lib.rs`); shot changes keep every scdet score so the cue stage chooses the
  threshold (review).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the file
  each stage writes.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage produces.

# Job model source

The `job_model` library: one module folder per kind of contract that stages write into a job's
work directory and read back: the stage and step names, the job record, each step's output and the
quality-check report.

## Contents

```text
crates/job_model/src/
├── onscreen/   typed visual observations, translations, corrections and presentation
├── job/         the job record kept in `job.json`: video, settings, finished steps and their measures
├── lib.rs       the crate root: the module list and the `StageName` and `StepName` re-exports
├── model_call/  one language-model call for the app's log window, and the worker line carrying it
├── outputs/     the typed output of each step, one JSON file per step in the work directory
├── report/      the quality check's result: every check, its findings and the summary counts
├── stage/       every stage and every step in run order, with the names they carry
└── tests/       the rkyv round trip every module's `tests/archive.rs` runs
```

## How it works

`stage/` names the stages and the steps they are made of; the job runner, the `worker`
subcommands and the work directory all use those names. `job/` holds the record of one job, keyed
by step, with the settings every step's fingerprint reads. `outputs/` holds what each step writes,
from the probe result to the aligned words, and `report/` the quality check's result that
`report.md` is rendered from. Everything here is plain data: no module reads a file, spawns a
process or holds state. Every data type derives serde, for the JSON files, and rkyv's `Archive`,
`Serialize` and `Deserialize`, for the job database of the
[binary storage plan](/documentation/architecture/binary_storage_plan.md); path fields archive as
UTF-8 strings.

## Public surface

- `StageName` and `StepName`, re-exported at the crate root from `stage`, with `UnknownStage` and
  `UnknownStep`: for `crates/pipeline/` and the app's command lines.
- `job`: `JobRecord`, `StepRecord`, `StepMeasure`, `WorkerMeasure`, `JobSettings`, `Separator`
  and `WhisperModel`, for `crates/pipeline/`, `crates/stages/src/qc/` and the app's `process`
  subcommand.
- `outputs`: the step outputs (`ProbeDecoded`, `ShotChanges`, `SpeechPlan`, `EngineTranscript`,
  `Utterance`, `SoundEvent`, `AdjudicationPass`, `Redecode`, `SoundCues`, `Aligned` and the types
  inside them), for `crates/media_io/`, `crates/inference/`, `crates/stages/`, `crates/pipeline/`
  and the stack spike tools.
- `report`: `QcCheck`, `QcFinding`, `QcSummary` and `QcReport`, for `crates/stages/src/qc/` and
  `crates/pipeline/`.
- `model_call`: `ModelExchange` and `WORKER_LINE_PREFIX`, for `crates/inference/src/llm/`,
  `crates/pipeline/` and the app's log window.

## Boundaries

- Depends on: `serde` and `rkyv` (with `unaligned`) for the derives.
- Used by: every other product crate and both app binaries.
- Rules: the JSON names never change once a step writes them, so a resumed job reads what an
  earlier run wrote (`json_names_match_the_command_line_names` in `stage/tests/stage_name.rs`,
  `steps_serialise_by_name` in `stage/tests/step_name.rs`); every data type round-trips through
  rkyv from a misaligned slice (`round_trip` in `tests/archive_round_trip.rs`, run by each
  module's `tests/archive.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the files
  of a job's work directory.
- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the steps and the
  file each writes.

# Job model source

The `job_model` library: one module folder per kind of contract that stages write into a job's
work directory and read back: the stage and step names, the job record, each step's output and the
quality-check report.

## Contents

```text
crates/job_model/src/
├── job/         the job record and step records: video, settings, finished steps and their measures
├── lib.rs       the crate root: the module list and the `StageName` and `StepName` re-exports
├── model_call/  one language-model call for the app's log window, whose JSON a worker sends it
├── onscreen/    the on-screen text documents, replacements, per-frame rows and library signs
├── outputs/     the typed output of each step, stored under the step's key in the job database
├── report/      the quality check's result: every check, its findings and the summary counts
├── stage/       every stage and every step in run order, with the names they carry
├── store/       the layout version of every table of the job database
└── tests/       the rkyv round trip every module's `tests/archive.rs` runs
```

## How it works

`stage/` names the stages and the steps they are made of; the job runner, the `worker`
subcommands and the job database's keys all use those names. `job/` holds the record of one
job, keyed by step, with the settings every step's fingerprint reads. `outputs/` holds what each
step writes, from the probe result to the output's record, `onscreen/` the on-screen text documents from
detection to the read-back check, `model_call/` one language-model call as the log window shows
it, `store/` the layout version of every table, and `report/` the quality check's result that
`report.md` is rendered from. Everything here is plain data: no module reads a file, spawns a
process or holds state. Every data type derives serde, for its JSON form, and rkyv's `Archive`,
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
  `Utterance`, `SoundEvent`, `AdjudicationPass`, `Redecode`, `SoundCues`, `Aligned`,
  `Corrections`, `FixRecord`, `OutputRecord` and the types inside them), for `crates/media_io/`,
  `crates/inference/`, `crates/stages/`, `crates/pipeline/` and the stack spike tools.
- `onscreen`: the visual documents (`TextDocument`, `TextCorrections`, `TextSettings`,
  `ReplacementDocument`, `FrameRecord`, `VerifiedReplacements`, `VerifyReading`,
  `LocalizedVideoRecord`, `LibrarySign`), for `crates/stages/src/onscreen_text/`,
  `crates/pipeline/`, the app's Check Text and `tools/visual_validation/`.
- `report`: `QcCheck`, `QcFinding`, `QcSummary` and `QcReport`, for `crates/stages/src/qc/` and
  `crates/pipeline/`.
- `model_call`: `ModelExchange`, for `crates/inference/src/llm/`,
  `crates/pipeline/` and the app's log window.
- `store`: `TableLayouts`, the layout version of every table of a job database, for
  `crates/pipeline/src/work_dir/store/`.

## Boundaries

- Depends on: `serde` and `rkyv` (with `unaligned`) for the derives.
- Used by: every other product crate and the three app binaries.
- Rules: the step names never change, since they spell the keys of a job's rows and their JSON,
  so a resumed job reads what an earlier run stored (`json_names_match_the_command_line_names`
  in `stage/tests/stage_name.rs`, `steps_serialise_by_name` in `stage/tests/step_name.rs`); every
  data type round-trips through rkyv from a misaligned slice (`round_trip` in
  `tests/archive_round_trip.rs`, run by each module's `tests/archive.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the
  layout of a job's work directory.
- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the steps and
  what each stores.

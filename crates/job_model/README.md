# Job model

The `job_model` crate: the types the pipeline's [stages](/documentation/glossary.md#stage) write
into a job's [work directory](/documentation/glossary.md#work-directory) and read back, which are
the contracts between them: the stage and step names, the job record, each step's output and the
quality-check report. Each is written as JSON today and also archived with rkyv for the job
database the [binary storage plan](/documentation/architecture/binary_storage_plan.md) describes.
It sits below every other product crate and depends on no workspace crate.

## Contents

```text
crates/job_model/
├── Cargo.toml  the `job_model` library package: `serde` with derive, `serde_json`, `rkyv`
└── src/        the stage and step names, the job record, the step outputs and the report
```

## How it works

`StageName` is the one list of the eleven stages in run order, and `StepName` the one list of the
twenty-nine steps they are made of, which the job runner runs, resumes and times one by one. Each
has one name, used on the command line, in file names and in JSON: `Display`, `FromStr` and
serde's `snake_case` all spell it. The app's `worker` subcommands and the `process` subcommand's
`--rerun` option parse step names through `FromStr`.

`job` holds the record kept in `job.json`: the video, the `JobSettings` the job runs with, and
each finished step's fingerprint, finish time and measurements. `outputs` holds what each step
writes, from `probe.json` through the transcripts, the diff sheet, the language model's passes and
the sound cues to `aligned.json`. `report` holds the quality check's result, kept in `qc.json` and
rendered into `report.md`. `src/README.md` describes each module.

Every public data type also derives rkyv's `Archive`, `Serialize` and `Deserialize` beside its
serde derives, so the job database can store it as a binary record and read it in place. The
`unaligned` feature gives every archived type alignment 1, so a record is read straight from a
redb value at any offset without a copy. The format is pinned in `Cargo.toml` rather than left to
rkyv's defaults: little-endian, with lengths, offsets and `usize` fields archived as 32-bit
integers. A `usize` field is cast to `u32` without a check; every one here is a count or an index
far below `u32::MAX`. rkyv cannot archive a `PathBuf` as such, so path fields
keep their Rust type and archive as a UTF-8 string (`rkyv::with::AsString`, through
`rkyv::with::Map` for `Option` and `Vec`); a path that is not UTF-8 makes archiving fail with an
error. Maps keep their order: the archived fieldless enums derive `Ord`, so `JobRecord::steps` and
`FixBefore::counts` archive as ordered maps searched in place by an archived key such as
`ArchivedStepName::Readjudicate`.

## Getting started

Run these from the repository root:

```bash
cargo build -p job_model   # the library
cargo test -p job_model    # 106 unit tests, every rkyv round trip included; well under a second
```

## Configuration

None: the crate reads no setting. `JobSettings` is data the pipeline stores, not configuration the
crate reads.

## Public surface

- The library `job_model`, with `StageName` and `StepName` re-exported at its root and the modules
  `stage` (`StageName`, `StepName`, their archived forms `ArchivedStageName` and
  `ArchivedStepName`, `UnknownStage`, `UnknownStep`), `job` (`JobRecord`,
  `StepRecord`, `StepMeasure`, `WorkerMeasure`, `JobSettings`, `Separator`, `WhisperModel`),
  `outputs` (the probe result, shot changes, speech plan, transcripts, sheet, sound events,
  adjudication passes, re-decodes, sound cues and aligned words) and `report` (`QcCheck`,
  `QcFinding`, `QcSummary`, `QcReport`).
- No binary.

## Boundaries

- Depends on: `serde` 1 with `std` and `derive`; `serde_json` 1 (a model call's worker line);
  `rkyv` 0.8.18 with `unaligned`, `little_endian` and `pointer_width_32` (the job database's
  binary records). No workspace crate.
- Used by:
  - the apps: `apps/tbd_subtitles/src/cli/` parses step names and builds `JobSettings`, and
    `apps/tbd_subtitles_ggml/src/main.rs` parses the step its worker runs;
  - `crates/pipeline/`, which keeps the job record, runs the steps and reads and writes every
    output;
  - `crates/stages/`, whose stages take and return the output types;
  - `crates/media_io/`, which returns the probe result and the shot changes;
  - `crates/inference/`, whose speech engines return timed words;
  - `crates/subtitle_formats/`, which declares it as a dependency and uses nothing from it yet;
  - the stack spike tools in `tools/`.
- Rules:
  - the crate sits in layer 0 and depends on no workspace crate (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - each stage and step name parses back to its own variant and is the same in JSON and on the
    command line, so a resumed job reads what an earlier run wrote
    (`every_stage_is_listed_once_and_parses_back_to_itself` and
    `json_names_match_the_command_line_names` in `crates/job_model/src/stage/tests/stage_name.rs`;
    `every_step_is_listed_once_and_parses_back` and `steps_serialise_by_name` in
    `crates/job_model/src/stage/tests/step_name.rs`);
  - every contract type round-trips through rkyv, read in place from a misaligned slice
    (`round_trip` in `crates/job_model/src/tests/archive_round_trip.rs`, run by the
    `tests/archive.rs` file of each module: `job`, `model_call`, `onscreen`, `outputs`, `report`
    and `stage`), and a path that is not UTF-8 fails to archive
    (`a_path_that_is_not_utf8_fails_to_archive` in
    `crates/job_model/src/onscreen/tests/archive.rs`);
  - an archived job record's steps are found in place by an archived step name
    (`archived_steps_are_found_by_archived_step_name` in
    `crates/job_model/src/job/tests/archive.rs`);
  - a type here changes only together with every stage that reads or writes it (review);
  - a change to a type's fields changes its rkyv layout, which nothing detects by itself (a
    change that keeps the size passes rkyv's validation), so whoever changes a contract type
    bumps the layout version of every table that stores it and the steps that wrote them rerun
    (review; "Adding a field" in the binary storage plan).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the job
  work directory and the file each stage writes.
- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the stages, their
  steps and the file each step writes.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md) — the job database
  these types are archived into with rkyv.

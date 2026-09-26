# Job model

The `job_model` crate: the serde types the pipeline's [stages](/documentation/glossary.md#stage)
write into a job's [work directory](/documentation/glossary.md#work-directory) and read back, which
are the contracts between them: the stage and step names, the job record, each step's output and
the quality-check report. It sits below every other product crate and depends on no workspace
crate.

## Contents

```text
crates/job_model/
├── Cargo.toml  the `job_model` library package: `serde` with derive, and `serde_json` for the tests
└── src/        the stage and step names, the job record, the step outputs and the report
```

## How it works

`StageName` is the one list of the eleven stages in run order, and `StepName` the one list of the
seventeen steps they are made of, which the job runner runs, resumes and times one by one. Each
has one name, used on the command line, in file names and in JSON: `Display`, `FromStr` and
serde's `snake_case` all spell it. The app's `worker` subcommands and the `process` subcommand's
`--rerun` option parse step names through `FromStr`.

`job` holds the record kept in `job.json`: the video, the `JobSettings` the job runs with, and
each finished step's fingerprint, finish time and measurements. `outputs` holds what each step
writes, from `probe.json` through the transcripts, the diff sheet, the language model's passes and
the sound cues to `aligned.json`. `report` holds the quality check's result, kept in `qc.json` and
rendered into `report.md`. `src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p job_model   # the library
cargo test -p job_model    # 10 unit tests for the names and the job record, well under a second
```

## Configuration

None: the crate reads no setting. `JobSettings` is data the pipeline stores, not configuration the
crate reads.

## Public surface

- The library `job_model`, with `StageName` and `StepName` re-exported at its root and the modules
  `stage` (`StageName`, `StepName`, `UnknownStage`, `UnknownStep`), `job` (`JobRecord`,
  `StepRecord`, `StepMeasure`, `WorkerMeasure`, `JobSettings`, `Separator`, `WhisperModel`),
  `outputs` (the probe result, shot changes, speech plan, transcripts, sheet, sound events,
  adjudication passes, re-decodes, sound cues and aligned words) and `report` (`QcCheck`,
  `QcFinding`, `QcSummary`, `QcReport`).
- No binary.

## Boundaries

- Depends on: `serde` 1 with `std` and `derive`; `serde_json` 1 in the tests only. No workspace
  crate.
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
  - a type here changes only together with every stage that reads or writes it (review).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the job
  work directory and the file each stage writes.
- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — the stages, their
  steps and the file each step writes.

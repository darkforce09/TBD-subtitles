# Job record

The job record kept in a job's `job.json`: the video it is for, the settings it runs with, and the
fingerprint, finish time and measurements of every finished step; plus what a worker process
reports about itself.

## Contents

```text
crates/job_model/src/job/
├── mod.rs       the module tree and the re-exports
├── record.rs    `JobRecord`, `StepRecord`, `StepMeasure` and `WorkerMeasure`
├── settings.rs  `JobSettings` and its model choices, `Separator` and `WhisperModel`
└── tests/       unit tests for the record's JSON
```

## How it works

A `JobRecord` names the video by path, size and modification time, holds its `JobSettings`, and
maps each finished `StepName` to a `StepRecord`: the fingerprint of the step's settings and
inputs, its finish time in nanoseconds, and its `StepMeasure` (wall time, load and process time,
peak memory of the step, of its largest child and of the GPU, and short notes). A measure that was
not taken is `None`, never zero. A worker writes its own `WorkerMeasure` when its step finishes,
and the runner folds it into the step's measure.

`JobSettings` is everything that changes a job's output: the separation model (`roformer` or
`mdx_net`), the Whisper model (`large_v3` or `large_v3_turbo`), the audio track, the glossary, the
`claude` model and how many processes run at once, and the lowest shot-change score that counts
as a cut. `JobSettings::with_glossary` gives the defaults: RoFormer, large-v3, the English track,
`sonnet` with 8 processes, and a cut score of 20.

## Boundaries

- Depends on: `serde`; `crate::stage::StepName`, the key of the step map.
- Used by: `crates/pipeline/` (the runner, the resume check, the step graph, the workers, the
  progress and the report); `crates/stages/src/qc/markdown.rs`, which renders the step timings;
  the app's `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs`, which builds
  the settings.
- Rules:
  - a record round-trips through JSON with its steps keyed by step name, and a record without
    steps parses (`a_record_round_trips_through_json_with_steps_by_name`,
    `a_record_without_steps_parses` in `tests/record.rs`);
  - the JSON names stay stable so a resumed job reads what an earlier run wrote (the crate header
    in `crates/job_model/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — how the fingerprint
  decides which steps resume, and what a worker measures.
- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — `job.json`
  among the files of a job's work directory.

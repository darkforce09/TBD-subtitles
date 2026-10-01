# Job record

The job record kept in a job's database (`meta/job_record`): the video it is for and the settings
it runs with; the step records (`step_records/<step>`): the fingerprint, finish time and
measurements of every finished step; plus what a worker process reports about itself.

## Contents

```text
crates/job_model/src/job/
├── mod.rs       the module tree and the re-exports
├── record.rs    `JobRecord`, `StepRecord`, `StepRecords`, `StepMeasure` and `WorkerMeasure`
├── settings.rs  `JobSettings`, its model choices `Separator` and `WhisperModel`, and `OutputFormat`
└── tests/       unit tests for the record's JSON and every type's rkyv round trip
```

## How it works

A `JobRecord` names the video by path, size and modification time, holds its `JobSettings`,
names the models folder the run read (`None` for the default; no fingerprint covers it), and the
digest of the owner's line corrections when the run started. Each finished step has a
`StepRecord` of its own: the fingerprint of the step's settings and inputs, its finish time in
nanoseconds, and its `StepMeasure` (wall time, load and process time, peak memory of the step, of
its largest child and of the GPU, and short notes); `StepRecords` maps each `StepName` to its
record, as the `step_records` table reads back. A measure that was not taken is `None`, never
zero. A worker sends its own `WorkerMeasure` when its step finishes, and the runner folds it into
the step's measure.

`JobSettings` is everything that changes a job's output: the separation model (`roformer` or
`mdx_net`), the Whisper model (`large_v3` or `large_v3_turbo`), the audio track, the glossary, the
`claude` model and how many processes run at once, the lowest shot-change score that counts as a
cut, and the subtitle file's format (`srt`, `vtt` or `ass`). `JobSettings::with_glossary` gives
the defaults: RoFormer, large-v3, the English track, `sonnet` with 8 processes, a cut score of 20
and SRT. The records are rkyv archives in the job's database; a changed type bumps its table's
layout version, and the steps run again (there is no migration).

## Boundaries

- Depends on: `serde`, `rkyv`; `crate::stage::StepName`, the key of `StepRecords`.
- Used by: `crates/pipeline/` (the runner, the resume check, the step graph, the workers, the
  progress and the report); `crates/stages/src/qc/markdown.rs`, which renders the step timings;
  the app's `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs`, which builds
  the settings.
- Rules:
  - a record round-trips through JSON, and the step records do with their steps keyed by step
    name (`a_record_round_trips_through_json`,
    `step_records_round_trip_through_json_by_step_name` in `tests/record.rs`);
  - every type round-trips through rkyv, and archived step records are found by archived step
    name (`tests/archive.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — how the fingerprint
  decides which steps resume, and what a worker measures.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#tables-in-jobredb) —
  the `meta` and `step_records` tables these records live in.

# Job runner source

The `pipeline` library: one module folder for each part of running a job, which are the runner,
the step graph, resume, the work directory, the worker processes, the step bodies, the
measurements, the progress events and the report.

## Contents

```text
crates/pipeline/src/
├── cancel.rs  `CancelToken`: the shared flag that stops a running job and its worker
├── error.rs   `PipelineError`: what was being done, why, and its kind: failed, cancelled or busy
├── fix_it/    Fix It on a finished job: the model's kept changes written into the corrections
├── graph/     the step table: inputs, placement, GPU use, revision, settings, timeout and outputs
├── lib.rs     the crate root: the module list, the crate header and the `run_job` re-exports
├── measure/   peak VRAM of a worker through NVML, peak RAM of a process and its children
├── models/    the models a job needs: each step's model folder, the required list, the missing ones
├── progress/  the events a running job reports and the sink they go to
├── report/    `report.md` from the quality check and the job record
├── resume/    step fingerprints and whether a recorded output is reusable
├── runner/    `run_job`: one video through every step, in order, with resume and the shot scan
├── tasks/     the body of every step, shared by the runner and the `worker` subcommands
├── work_dir/  the path of every job file, the job id, complete JSON writes and the job store
└── workers/   starting a step's worker binary, its inputs and outputs, its frames and progress
```

## How it works

```text
runner ──▶ resume ──▶ graph            is the step's record still valid?
   │
   ├─ graph::placement = InProcess  ──▶ tasks::in_process ──▶ tasks::run ──▶ stages
   └─ graph::placement = Worker(b)  ──▶ workers::run_worker
                                          └─ `<b> worker <step> <job dir>`
                                               └─ tasks::worker_main ──▶ tasks::run
   │
   ├─ after each step: its outputs committed with its StepRecord in job.redb, job.json through
   │  work_dir, Progress::StepFinished
   └─ at the end: report::write ──▶ report.md
```

`runner` owns the loop and the job record. `graph` is the one table every other module asks: what
a step reads, where it runs, whether it needs the GPU, which settings it depends on, how long it
may run and which files it leaves. `resume` hashes that into a fingerprint. `work_dir` names
every path and writes JSON through a part file, so a file that exists is complete; its `store`
owns `job.redb`, which the runner and Fix It hold open while they work (one process at a time,
one shared handle in it, its pid in `job.lock`), with the record kind of every row
(`work_dir::store::kinds`), and which no step writes to yet. `tasks` holds the body of each step;
a worker binary calls `tasks::worker_main`, which sends `Progress`, `ModelCall`, `Measure` and
`Done` (or `Failed`) frames of the worker channel (`crates/worker_channel/`) on its stdout, and
`workers` turns those into progress events and a `StepMeasure`, adding the VRAM that
`measure::gpu_monitor` sampled. `workers::channel` carries a step's stored inputs down its
worker's stdin and its `Output` frames straight into the job database, uncommitted until the
runner commits them with the step's record; no step sends either yet. `report` renders `report.md` after every run. Every fallible call
returns `PipelineError`, whose `ErrorKind` says whether it failed, was cancelled, or found the job
owned by another process (`Busy`, with that process's pid when known).

`fix_it` works beside the runner, on a finished job: it holds the job's store, reads the job's
outputs, runs `stages::fix_it` with a `claude` backend the cancel token stops, keeps each answered call in
`fix/calls/`, writes `fix.json`, and puts the kept changes into `review.json` through
`work_dir::update_corrections`. The caller then runs the job again, and the corrections' digest
makes only the review step and the steps after it run.

## Public surface

- `run_job`, `JobOptions`, `JobOutcome`, `CancelToken`, `PipelineError` and `Result`, re-exported
  at the crate root for the app's `process` subcommand and its window.
- `tasks::worker_main` and `graph::{placement, Placement, Binary}`: the `worker` subcommands of
  `apps/tbd_subtitles/` and `apps/tbd_subtitles_ggml/`.
- `progress::Progress`, `workers::Binaries`, `work_dir::default_root` and
  `work_dir::gpu_lock_path`: what the app shows, the binaries it passes, its default work root and
  the GPU lock file.
- `models::{required, missing, default_dir}`: the model folders a job needs, for the window's
  models view and the check before a job starts.
- `fix_it::{fix_video, FixOptions, FixProgress, FixStage, FixOutcome}`: Fix It, for the window
  and the `fix` subcommand; `work_dir::update_corrections`: the window's line review.
- `measure::gpu_monitor` and `measure::memory`: used by `tools/stack_spike/`.
- `work_dir::JobStore` and `error::ErrorKind`: the job database the runner owns, and the busy kind
  the window turns into a busy job; `tools/visual_validation/` holds a `JobStore` too.
- `work_dir::store::{kind, RecordKind, kinds::shown}`: the record kind of a row, which checks its
  archive and prints it as JSON, for the app's `dump` subcommand.
- `workers::{run_worker, WorkerData, WorkerRun, StepWrite}`: a step in its worker with its inputs
  and uncommitted outputs, for the runner and `tools/visual_validation/`.

## Boundaries

- Depends on: `stages`, `inference`, `media_io`, `subtitle_formats`, `child_process`,
  `job_model`, `worker_channel`, `redb`, `serde_json`, `rkyv`, `sha2`, `libc` and
  `nvml-wrapper`.
- Used by: `apps/tbd_subtitles/src/cli/`, `apps/tbd_subtitles_ggml/src/main.rs`,
  `tools/stack_spike/src/measure/` and `tools/visual_validation/src/pilot.rs`.
- Rules:
  - no module but `runner` writes the job record, and it writes it after each finished step only
    (the header of `runner/mod.rs`);
  - every step's placement, inputs and outputs come from `graph`, never from a second table
    (`graph/tests/graph.rs`);
  - every JSON file goes through a part file and a rename
    (`json_round_trips_and_leaves_no_part_file` in `work_dir/tests/work_dir.rs`);
  - a worker's outputs are stored only with its step's record, in one transaction, and a step that
    does not finish stores nothing (`workers/channel/tests/channel.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the work
  directory and when a stage is skipped.
- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stages the steps run.

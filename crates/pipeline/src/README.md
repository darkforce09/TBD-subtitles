# Job runner source

The `pipeline` library: one module folder for each part of running a job, which are the runner,
the step graph, resume, the work directory, the worker processes, the step bodies, the
measurements, the progress events and the report.

## Contents

```text
crates/pipeline/src/
├── error.rs   `PipelineError`: what was being done and why it failed, and the `Context` helper
├── graph/     the step table: inputs, placement, GPU use, revision, settings, timeout and outputs
├── lib.rs     the crate root: the module list, the crate header and the `run_job` re-exports
├── measure/   peak VRAM of a worker through NVML, peak RAM of a process and its children
├── progress/  the events a running job reports and the sink they go to
├── report/    `report.md` from the quality check and the job record
├── resume/    step fingerprints, whether a recorded output is reusable, and the job lock
├── runner/    `run_job`: one video through every step, in order, with resume and the shot scan
├── tasks/     the body of every step, shared by the runner and the `worker` subcommands
├── work_dir/  the path of every job file, the job id, the default root and complete JSON writes
└── workers/   starting a step's worker binary, forwarding its progress and reading its measure
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
   ├─ after each step: resume::fingerprint, job.json through work_dir, Progress::StepFinished
   └─ at the end: report::write ──▶ report.md
```

`runner` owns the loop and the job record. `graph` is the one table every other module asks: what
a step reads, where it runs, whether it needs the GPU, which settings it depends on, how long it
may run and which files it leaves. `resume` hashes that into a fingerprint and holds the
`job.lock`. `work_dir` names every path and writes JSON through a part file, so a file that exists
is complete. `tasks` holds the body of each step; a worker binary calls `tasks::worker_main`,
which prints `progress <done> <total>` lines and writes `steps/<step>.worker.json`, and
`workers` turns those into progress events and a `StepMeasure`, adding the VRAM that
`measure::gpu_monitor` sampled. `report` renders `report.md` after every run. Every fallible call
returns `PipelineError`.

## Public surface

- `run_job`, `JobOptions`, `JobOutcome`, `PipelineError` and `Result`, re-exported at the crate
  root for the app's `process` subcommand.
- `tasks::worker_main` and `graph::{placement, Placement, Binary}`: the `worker` subcommands of
  `apps/tbd_subtitles/` and `apps/tbd_subtitles_ggml/`.
- `progress::Progress`, `workers::Binaries` and `work_dir::default_root`: what the `process`
  subcommand prints, the binaries it passes, and its default work root.
- `measure::gpu_monitor` and `measure::memory`: used by `tools/stack_spike/`.

## Boundaries

- Depends on: `stages`, `inference`, `media_io`, `subtitle_formats`, `child_process`,
  `job_model`, `serde_json`, `sha2`, `libc` and `nvml-wrapper`.
- Used by: `apps/tbd_subtitles/src/cli/`, `apps/tbd_subtitles_ggml/src/main.rs` and
  `tools/stack_spike/src/measure/`.
- Rules:
  - no module but `runner` writes the job record, and it writes it after each finished step only
    (the header of `runner/mod.rs`);
  - every step's placement, inputs and outputs come from `graph`, never from a second table
    (`graph/tests/graph.rs`);
  - every JSON file goes through a part file and a rename
    (`json_round_trips_and_leaves_no_part_file` in `work_dir/tests/work_dir.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the work
  directory and when a stage is skipped.
- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stages the steps run.

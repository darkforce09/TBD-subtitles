# Step runner

`run_job`: one video through every step, in the order `StepName::ALL` gives, skipping what is still
valid, running the shot scan beside the other steps, recording each step's measure, and writing
the report at the end.

## Contents

```text
crates/pipeline/src/runner/
└── mod.rs  `run_job`, `JobOptions` and `JobOutcome`, the CUDA environment and the step records
```

## How it works

`run_job` canonicalises the video, names its work directory with `work_dir::job_id`, opens the
job's database with `JobStore::open` and holds it until it returns (another process running the
job makes that open fail with the busy error kind, which names the owner's pid from `job.lock`),
and opens `job.json`, or starts a new record when there is none or it belongs to
another video. It records the video's size and modification time and this run's settings, drops
the steps named in `JobOptions::rerun`, and saves the record. `Progress::JobStarted` names the
steps this run will do (`resume::stale_steps`), and `Progress::JobDuration` the video's length once
the probe is there. It then walks the steps, checking `JobOptions::cancel` before each: a valid
step is skipped; any other runs through `tasks::in_process` or `workers::run_worker`, as
`graph::placement` says, and is recorded with its fingerprint, finish time and measure; a step
that fails, or is cancelled, is reported as `Progress::StepFailed` and ends the job with its error.
A worker gets the cancel token, which its watchdog watches, and a GPU worker first takes the
machine-wide lock `JobOptions::gpu_lock`. The shot
scan runs on a scoped thread and is joined before cue building, the first step that reads it; when
the walk ends with an error the runner sets the cancel token, so the scan stops instead of running
to its end. The
CUDA environment, found once through `inference::cuda_runtime` beside the binaries or in the
runtime folder, goes to GPU workers only. At the end `report::write` renders `report.md`, and
`JobOutcome` names the subtitle file, the report, the quality check and the steps run and skipped.

## Boundaries

- Depends on: `crate::{graph, resume, tasks, workers, work_dir, report, progress}`;
  `inference::cuda_runtime` and `inference::model_store`; `job_model`; `stages::output`; `tracing`
  for the `step{step}` span each step runs in (its workers' and programs' lines, and the shot
  scan's thread, log inside it), its debug lines (the job, reruns asked for, where each step runs,
  the CUDA runtime) and the report's path at info.
- Used by: `apps/tbd_subtitles/src/cli/process_command.rs` and the window's job queue, through
  the re-export at the crate root.
- Rules:
  - `job.json` holds finished steps only and is saved after each, so a killed job resumes from the
    last finished step (the module header);
  - one worker loads the GPU at a time; the shot scan, which runs beside it, loads none;
  - a step starts only after every step it reads has finished, and none starts once the job is
    cancelled;
  - the shot scan's thread stays alive until its worker is reaped, since a child dies with the
    thread that started it (`crates/child_process/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow the runner walks.
- [System overview](/documentation/architecture/system_overview.md#processes) — the processes a job
  starts.

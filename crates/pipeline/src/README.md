# Job runner source

The `pipeline` library: one module folder for each part of running a job, which are the stage
graph, resume, the work directory, the worker processes and the progress events. The modules are
not written yet.

## Contents

```text
crates/pipeline/src/
├── graph/     the stage graph: which stages run, in which order, on which inputs
├── lib.rs     the crate root: the module list and the crate header
├── progress/  progress events: current stage, stage progress, elapsed and remaining time
├── resume/    skipping a stage whose output exists and whose inputs and settings are unchanged
├── work_dir/  the job's work directory: its layout, each stage's output path, and `job.json`
└── workers/   starting a GPU stage as a `worker` subprocess, one at a time, and reading its result
```

## How it works

`graph/` is for the order and inputs of the stages, `resume/` for deciding which may be skipped,
`work_dir/` for where each output lives, `workers/` for starting the stages that run in a worker
process, and `progress/` for what the window and the command line show while a job runs. Each
module holds only a `mod.rs` with its header.

## Public surface

- `graph`, `progress`, `resume`, `work_dir` and `workers`: public modules with no items yet, for
  the app's `process` subcommand and window in `apps/tbd_subtitles/`.

## Boundaries

- Depends on: nothing yet; the crate declares `stages`, `child_process` and `job_model` for these
  modules.
- Used by: nothing yet; `apps/tbd_subtitles/Cargo.toml` declares the crate as a dependency.
- Rules: one GPU worker runs at a time, and a killed job leaves every finished stage valid for
  resume (the crate header in `lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the work
  directory and when a stage is skipped.

# Job runner

The `pipeline` crate: runs one job end to end, which means the
[stages](/documentation/glossary.md#stage) in graph order, skipping those whose outputs are still
valid, each GPU stage as a `worker` subprocess of the app binary, with progress events for the
window and the command line. Its modules are not written yet.

## Contents

```text
crates/pipeline/
├── Cargo.toml  the `pipeline` library package; depends on `stages`, `child_process` and `job_model`
└── src/        the stage graph, resume, the work directory, worker processes and progress events
```

## How it works

```text
app: `process` subcommand or the window
  └─▶ pipeline
        ├─ graph     which stages run, in which order, on which inputs
        ├─ resume    skip a stage whose output and recorded inputs are unchanged
        ├─ work_dir  the job's work directory: its layout, each stage's output, job.json
        ├─ workers   `tbd-subtitles worker <stage> <job dir>`, one at a time, via child_process
        └─ progress  current stage, stage progress, elapsed and remaining time
```

A CPU stage runs in process through `stages`; a stage for which `StageName::runs_in_worker` is true
runs as a [worker process](/documentation/glossary.md#worker-process), so its GPU memory and native
libraries go when it exits. Each module holds only its header; the runner is not written yet.
`src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p pipeline   # the module declarations, with stages and the crates beneath it
cargo test -p pipeline    # runs 0 tests: no module holds code yet
```

## Configuration

None: the crate reads no setting.

## Public surface

- The library `pipeline`, with the public modules `graph`, `progress`, `resume`, `work_dir` and
  `workers`; they hold no items yet.
- No binary.

## Boundaries

- Depends on: `stages`, `child_process` and `job_model`, declared in `Cargo.toml` and not called
  yet.
- Used by: the app, which declares it in `apps/tbd_subtitles/Cargo.toml`; no app code calls it yet.
- Rules:
  - the crate sits in layer 3 and depends only on lower layers (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - one GPU worker runs at a time, and a killed job leaves every finished stage valid for resume
    (the crate header in `crates/pipeline/src/lib.rs`; no test holds these yet).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#processes) — the processes and
  the job work directory.
- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow the runner walks.
- [Decisions](/documentation/decisions.md) — why each GPU stage runs in its own worker process.

# Job runner

The `pipeline` crate: runs one video through every step of the pipeline, in the order
`StepName::ALL` gives, skipping each step whose output is still valid, running each model step as a
[worker process](/documentation/glossary.md#worker-process) of one of the app's two binaries, and
recording the time and peak memory of every step. The app's `process` and `worker` subcommands and
the ggml worker binary call it.

## Contents

```text
crates/pipeline/
├── Cargo.toml  the `pipeline` package, its `crispasr` feature, and what each dependency is for
└── src/        the runner, the step graph, resume, work directory, workers, tasks, measures, report
```

## How it works

```text
tbd-subtitles process <video>
  └─▶ runner::run_job
        ├─ work_dir   job id from the video's path, job.json, every output path, and the
        │             JobStore that owns job.redb (job.lock names the owning process)
        ├─ resume     fingerprint each step; reuse it when the record matches and the files exist
        ├─ graph      inputs, placement, GPU flag, settings, timeout and outputs of each step
        ├─ in process ──▶ tasks::in_process ──▶ stages
        ├─ worker     ──▶ workers::run_worker ──▶ `<binary> worker <step> <job dir>`
        │                   ▲ frames on stdout      └─▶ tasks::worker_main ──▶ stages, inference
        ├─ measure    peak RAM of this process and its children; peak VRAM per worker through NVML
        ├─ progress   events to the caller's sink
        └─ report     report.md from qc.json and the job record
```

`run_job` canonicalises the video, derives the job's folder under the work root, opens the job's
database (`work_dir::JobStore`, which one process owns at a time; another process running the job
is a busy error naming its pid) and writes `job.json` with the settings of this run. No step
writes to the database yet: every output is still a JSON file. It then walks the steps: a step whose
recorded fingerprint and output files are intact is skipped; any other runs, in process (voice
activity, the diff sheet, cue building, the quality check and the output) or in a worker of
`tbd-subtitles` (FFmpeg, ONNX Runtime and the `claude` CLI) or `tbd-subtitles-ggml` (Whisper). The
shot scan runs on a scoped thread beside the other steps and is joined before the first step that
reads it. After every step `job.json` records its fingerprint, finish time and measure, so a
killed job resumes from the last finished step. The step's code lives in `tasks`, which both
binaries share, so the same body runs in the runner or in a worker. A worker reports to the runner
only in frames of the worker channel (`crates/worker_channel/`) on its stdout: its progress, its
model calls, its measure and its end or failure; its stderr is the step's log. `src/README.md`
describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p pipeline   # the library, with stages, inference and the crates beneath it
cargo test -p pipeline    # 76 unit tests: the graph, resume, the work directory, the job store, worker frames
```

A whole job runs through the app: build both binaries and run
`tbd-subtitles process <video>` on the host, as the
[development environment runbook](/documentation/runbooks/development_environment.md) says. The
worker binaries must sit beside the running binary.

## Configuration

The crate reads no settings file. What changes a job's output is the `JobSettings` the caller
passes in `JobOptions`, recorded in the job's `job.json` (`crates/job_model/src/job/settings.rs`);
`graph::settings` picks the part each step reads. Other inputs:

- the work root: `JobOptions::work_root`, by default `<data home>/tbd-subtitles/work` from
  `work_dir::default_root`, where the data home is `XDG_DATA_HOME` or `~/.local/share` (read by
  `crates/inference/src/model_store/mod.rs`); the models and the CUDA runtime folder sit beside it;
- `LD_LIBRARY_PATH`, which `inference::cuda_runtime` extends, with `ORT_DYLIB_PATH`, for every GPU
  worker;
- the Cargo feature `crispasr`, off by default: it builds the Whisper tasks, and only
  `apps/tbd_subtitles_ggml/` turns it on; without it a Whisper step fails instead of being skipped;
- NVML (`libnvidia-ml.so`), loaded at run time when present; without it no VRAM is recorded.

## Public surface

- `run_job`, `JobOptions`, `JobOutcome` and `CancelToken` at the crate root: one job end to end,
  stoppable, for the `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs` and
  the window's job queue.
- `tasks::worker_main`: the body of the `worker` subcommand of both binaries
  (`apps/tbd_subtitles/src/cli/worker_command.rs`, `apps/tbd_subtitles_ggml/src/main.rs`).
- `graph::{placement, Placement, Binary}`: which binary a step's worker runs in, which the
  `worker` subcommands check before they start.
- `progress::{Progress, ProgressSink}`, `workers::Binaries`, `work_dir::default_root` and
  `work_dir::gpu_lock_path`: the events the caller shows, the binaries beside the running one, the
  default work root and the machine-wide GPU lock file.
- `measure::{gpu_monitor, memory}`: the VRAM sampler and the peak-memory readings, also used by
  `tools/stack_spike/`.
- `work_dir::JobStore` with `work_dir::store::{StoreRead, StoreWrite, LAYOUT_VERSIONS}`: the job
  database a job runner owns.
- `PipelineError`, `error::ErrorKind` and `Result`: what failed, what was being done, and whether
  it failed, was cancelled or found the job busy in another process.
- No binary.

## Boundaries

- Depends on: `stages`, `inference`, `media_io`, `subtitle_formats`, `child_process`,
  `job_model` and `worker_channel`; `redb` 4.3.0 (the job database), `serde`, `serde_json`,
  `rkyv` (the worker's measure and the database's rows), `sha2`, `libc` (`getrusage`),
  `nvml-wrapper` and `tracing`
  (debug lines on reruns, placement, the CUDA runtime, the GPU lock and the report); at run time
  the app's two binaries as workers, and through them FFmpeg and the `claude` CLI.
- Used by: `apps/tbd_subtitles/` (the `process` and `worker` subcommands),
  `apps/tbd_subtitles_ggml/` (its `worker` subcommand) and `tools/stack_spike/` (the measures).
- Rules:
  - the crate sits in layer 3 and depends only on lower layers (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - a step reads only earlier steps, every GPU step runs in a worker, and only the Whisper steps
    run in the ggml binary, so ONNX Runtime and ggml never share a process
    (`crates/pipeline/src/graph/tests/graph.rs`);
  - one worker runs at a time besides the shot scan, which loads no GPU, and `job.json` holds
    finished steps only, so a killed job resumes from the last one (the header of
    `crates/pipeline/src/runner/mod.rs`);
  - a changed setting, video or upstream step reruns exactly the steps that read it, and a missing
    output reruns its step (`crates/pipeline/src/resume/tests/resume.rs`);
  - one process owns a job's database, and every caller in it shares one handle
    (`crates/pipeline/src/work_dir/store/tests/store.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow the runner walks.
- [System overview](/documentation/architecture/system_overview.md#processes) — the processes and
  the job work directory.
- [Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process)
  — why models load in workers.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why Whisper runs in `tbd-subtitles-ggml`.

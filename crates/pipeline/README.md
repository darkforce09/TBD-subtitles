# Job runner

The `pipeline` crate: runs one video through every step of the pipeline, in the order
`StepName::ALL` gives, skipping each step whose output is still valid, running each model step as a
[worker process](/documentation/glossary.md#worker-process) of one of the app's three binaries, and
recording the time and peak memory of every step. The app's `process` and `worker` subcommands and
the ggml and local-model worker binaries call it.

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
        ├─ work_dir   job id from the video's path, every file path, and the JobStore that owns
        │             job.redb (job.lock names the owning process): job record, step records,
        │             outputs, corrections
        ├─ resume     fingerprint each step; reuse it when its record matches, its documents are
        │             stored and the files its rows name exist
        ├─ graph      inputs, stored reads, dependents, placement, GPU flag, settings, timeout
        ├─ in process ──▶ tasks::in_process(StepIo over the store) ──▶ stages
        ├─ worker     ──▶ workers::run_worker ──▶ `<binary> worker <step> <job dir>`
        │                   │ inputs on stdin       └─▶ tasks::worker_main ──▶ stages, inference
        │                   ▲ frames on stdout, outputs straight into job.redb
        ├─ measure    peak RAM and per-worker VRAM; per step the whole job's CPU, GPU and memory
        ├─ progress   events to the caller's sink
        └─ report     report.md from outputs/qc, the job record, the step records and the last run
```

`run_job` canonicalises the video, derives the job's folder under the work root, opens the job's
database (`work_dir::JobStore`, which one process owns at a time; another process running the job
is a busy error naming its pid; the open removes the files no row names) and, in one transaction,
puts the job record of this run and clears the steps `--rerun` names with every step that reads
them. A task reads and writes stored documents through its `tasks::StepIo`: in the runner from a
snapshot and into the step's pending write, in a worker from the `Input` frames the runner sends
down its stdin (`graph::reads`) and as `Output` frames that go straight into the step's write
transaction, checked against their record kind. Every step keeps its documents only there; the
files that stay outside (audio streams, crops, keyframes, masks, plates, patches) are synced
before the record that names them commits. It then walks the steps: a step whose stored
fingerprint, documents and named files are intact is skipped; any other runs, in process (voice
activity, the diff sheet, cue building, the on-screen text review, the quality check and the
output) or in a worker of `tbd-subtitles` (FFmpeg, ONNX Runtime and the `claude` CLI),
`tbd-subtitles-ggml` (Whisper) or `tbd-subtitles-llm` (the on-screen translation). The
shot scan runs on a scoped thread beside the audio steps and is joined before the first step that
reads it; the visual lane (text detection, reading and tracking) runs on another beside
adjudication and the audio steps after it, and is joined before translation. Every step's outputs are committed with its step record (fingerprint, finish time and
measure) in one transaction, and a step about to run again loses its record first, so a killed
job resumes from the last finished step. The step's code lives in `tasks`, which both
binaries share, so the same body runs in the runner or in a worker. A worker reports to the runner
only in frames of the worker channel (`crates/worker_channel/`) on its stdout: its progress, its
model calls, its measure and its end or failure; its stderr is the step's log. `src/README.md`
describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p pipeline   # the library, with stages, inference and the crates beneath it
cargo test -p pipeline    # 152 unit tests: graph, resume, store, record kinds, channel, library
```

A whole job runs through the app: build the three binaries and run
`tbd-subtitles process <video>` on the host, as the
[development environment runbook](/documentation/runbooks/development_environment.md) says. The
worker binaries must sit beside the running binary.

## Configuration

The crate reads no settings file. What changes a job's output is the `JobSettings` the caller
passes in `JobOptions`, recorded in the job record (`crates/job_model/src/job/settings.rs`);
`graph::settings` picks the part each step reads. Other inputs:

- the work root: `JobOptions::work_root`, by default `<data home>/tbd-subtitles/work` from
  `work_dir::default_root`, where the data home is `XDG_DATA_HOME` or `~/.local/share` (read by
  `crates/inference/src/model_store/mod.rs`); the models and the CUDA runtime folder sit beside it;
- the sign library: `JobOptions::library`, which the app sets to
  `<data home>/tbd-subtitles/library.redb` from `library::default_path` (`None` runs without
  one); the runner names it to the workers that read it in
  `TBD_SUBTITLES_LIBRARY` (`library::LOCATION_VARIABLE`, read in `src/library/mod.rs`);
- `LD_LIBRARY_PATH`, which `inference::cuda_runtime` extends, with `ORT_DYLIB_PATH`, for every GPU
  worker;
- the Cargo feature `crispasr`, off by default: it builds the Whisper tasks, and only
  `apps/tbd_subtitles_ggml/` turns it on; without it a Whisper step fails instead of being skipped;
- NVML (`libnvidia-ml.so`), loaded at run time when present; without it no VRAM is recorded.

## Public surface

- `run_job`, `JobOptions`, `JobOutcome` and `CancelToken` at the crate root: one job end to end,
  stoppable, for the `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs` and
  the window's job queue.
- `tasks::{in_process, StepIo, Job}`: one step's task on its stored inputs and outputs, for the
  runner and `tools/visual_validation/`; `resume::{fingerprint, is_valid, stale_steps}` and
  `runner::worker_inputs` for the same tool.
- `tasks::worker_main`, `tasks::worker_main_from` and `tasks::refuse_terminal_stdin`: the body of
  the `worker` subcommand of all three binaries, which refuses a terminal on stdin
  (`apps/tbd_subtitles/src/cli/worker_command.rs` through `worker_main_from`,
  `apps/tbd_subtitles_ggml/src/main.rs`, `apps/tbd_subtitles_llm/src/main.rs`).
- `graph::{placement, Placement, Binary}`: which binary a step's worker runs in, which the
  `worker` subcommands check before they start.
- `progress::{Progress, ProgressSink}`, `workers::Binaries`, `work_dir::default_root` and
  `work_dir::gpu_lock_path`: the events the caller shows, the binaries beside the running one, the
  default work root and the machine-wide GPU lock file.
- `measure::{gpu_monitor, memory}`: the VRAM sampler and the peak-memory readings, also used by
  `tools/stack_spike/`.
- `work_dir::JobStore` with `work_dir::store::{StoreRead, StoreWrite, LAYOUT_VERSIONS}`: the job
  database a job runner owns; `work_dir::store::{kind, RecordKind}`: the type of every row, which
  checks an archive and prints it as JSON, for the app's `dump` subcommand;
  `work_dir::store::keys`: the name of every row; `work_dir::{load_job_record, load_step_records,
  read_job, read_stored}` and the `JobStore::put_*` fixtures: the job's rows for the app's
  readers and tests; `work_dir::{read_corrections, update_corrections, corrections_digest,
  read_text_corrections, update_text_corrections, read_fix_record, put_fix_record}`: the owner's
  corrections and Fix It's record.
- `workers::{run_worker, WorkerData, WorkerRun, StepWrite}`: one step in its worker, with its
  inputs from the job database and its outputs uncommitted, for the runner and
  `tools/visual_validation/`.
- `PipelineError`, `error::ErrorKind` and `Result`: what failed, what was being done, and whether
  it failed, was cancelled or found the job busy in another process.
- No binary.

## Boundaries

- Depends on: `stages`, `inference`, `media_io`, `subtitle_formats`, `child_process`,
  `job_model` and `worker_channel`; `redb` 4.3.0 (the job database), `serde`, `serde_json`,
  `rkyv` (the worker's measure and the database's rows), `sha2`, `image` and
  `unicode-normalization` (the sign library's crop hash and key), `libc` (`getrusage`),
  `nvml-wrapper` and `tracing`
  (debug lines on reruns, placement, the CUDA runtime, the GPU lock and the report); at run time
  the app's three binaries as workers, and through them FFmpeg and the `claude` CLI.
- Used by: `apps/tbd_subtitles/` (the `process`, `fix`, `dump` and `worker` subcommands and the
  window), `apps/tbd_subtitles_ggml/` and `apps/tbd_subtitles_llm/` (their `worker` subcommands),
  `tools/stack_spike/` (the measures) and `tools/visual_validation/` (the visual pilot's steps).
- Rules:
  - the crate sits in layer 3 and depends only on lower layers (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - a step reads only earlier steps, every GPU step runs in a worker, and only the Whisper steps
    run in the ggml binary, so ONNX Runtime and ggml never share a process
    (`crates/pipeline/src/graph/tests/graph.rs`);
  - one GPU worker runs at a time, the visual lane's included, and a step record exists
    only beside the outputs it was committed with, so a killed job resumes from the last finished
    step (the header of `crates/pipeline/src/runner/mod.rs`);
  - a changed setting, video or upstream step reruns exactly the steps that read it, and a missing
    output reruns its step (`crates/pipeline/src/resume/tests/resume.rs`);
  - one process owns a job's database, and every caller in it shares one handle
    (`crates/pipeline/src/work_dir/store/tests/store.rs`);
  - a worker's outputs are stored with its step's record in one transaction, or not at all
    (`crates/pipeline/src/workers/channel/tests/channel.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow the runner walks.
- [System overview](/documentation/architecture/system_overview.md#processes) — the processes and
  the job work directory.
- [Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process)
  — why models load in workers.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why Whisper runs in `tbd-subtitles-ggml`.

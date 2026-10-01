# Step runner

`run_job`: one video through every step, in the order `StepName::ALL` gives, skipping what is still
valid, running the shot scan beside the other steps, committing each step's outputs with its
record, and writing the report at the end.

## Contents

```text
crates/pipeline/src/runner/
├── mod.rs    `run_job`, `JobOptions`, `JobOutcome`, the CUDA environment, `worker_inputs`, `stamp`
├── rerun.rs  the job record and `--rerun` cleared in one transaction; a record forgotten to rerun
└── tests/    unit tests for the cleared steps, the per-frame rows and a forgotten record
```

## How it works

`run_job` canonicalises the video, names its work directory with `work_dir::job_id`, opens the
job's database with `JobStore::open` and holds it until it returns (another process running the
job makes that open fail with the busy error kind, which names the owner's pid from `job.lock`).
It builds this run's `JobRecord` (the video's path, size and modification time, the settings, the
models folder and the digest of the stored line corrections) and `rerun::start` commits it as
`meta/job_record` in the same write transaction that clears every step `JobOptions::rerun` names
and every step `graph::dependents` gives for them: their documents in `outputs` and their records
in `step_records`, and the rows of the per-frame table each of them owns (`graph::writes_rows`:
every `frames` row when `text_mask` is among them, every `readings` row when `text_verify` is); a
cleared `localized_video` keeps only the path of the video it wrote, as its earlier one, so its
rerun may replace that file. The files those rows named become unnamed and go on the database's
next open. No file outside the database holds the job record. `Progress::JobStarted` names the
steps this run will do (`resume::stale_steps`), and `Progress::JobDuration` the video's length once
the probe is stored.

It then walks the steps, checking `JobOptions::cancel` before each: a valid step
(`resume::is_valid` over a fresh snapshot) is skipped; any other has its fingerprint taken, its
record and the rows of the per-frame table it owns removed (`rerun::forget`), and runs through `tasks::in_process` on a `StepIo` of the store
or through `workers::run_worker` with the stored values `worker_inputs` gives it (every
`graph::reads` value, less the optional ones the job lacks), as `graph::placement` says. Either
way the outputs the step wrote come back as an uncommitted `StepWrite`, and `stamp` commits them
with its `StepRecord` (fingerprint, finish time, measure) in one transaction; a step that wrote
none still commits its record. A step that fails, or is cancelled, is reported as
`Progress::StepFailed` and ends the job with its error. A worker gets the cancel token, which its
watchdog watches, and a GPU worker first takes the machine-wide lock `JobOptions::gpu_lock`. The
shot scan runs on a scoped thread, commits its own outputs there, and is joined before the first
step that reads it; when the walk ends with an error the runner sets the cancel token, so the
scan stops instead of running to its end. The CUDA environment, found once through
`inference::cuda_runtime` beside the binaries or in the runtime folder, goes to GPU workers only.
The sign library `JobOptions::library` names (`None` for none) reaches the fingerprints
(`resume::is_valid`, `resume::fingerprint`), the in-process tasks through `tasks::Job::library`,
and the workers of `text_translate` and `text_compose` as `library::LOCATION_VARIABLE`, empty when
there is none.
Before the walk the runner starts one `measure::job_sampler::JobSampler` on its own pid; it opens
a step's window just before the step runs (on the shot scan's thread too) and closes it right
after, and the step's `StepUse` (CPU, GPU, NVENC, NVDEC and whole-job memory while it ran) goes
into its measure before `stamp`. When the walk ends the sampler stops, and the run's start, end
and peak memory are put as `meta/last_run` (`JobRun`). Then `report::write` renders `report.md`
from the record, the stored step records and the last run, and `JobOutcome` names the subtitle
file, the report, the quality check, the steps run and skipped, and the run.

## Boundaries

- Depends on: `crate::{graph, library, measure, resume, tasks, workers, work_dir, report,
  progress}`;
  `inference::cuda_runtime` and `inference::model_store`; `job_model`; `stages::output`;
  `worker_channel::address`; `tracing` for the `step{step}` span each step runs in (its workers'
  and programs' lines, and the shot scan's thread, log inside it), its debug lines (the job,
  reruns asked for and cleared, where each step runs, the CUDA runtime) and the report's path at
  info.
- Used by: the app's `process` and `fix` subcommands (`apps/tbd_subtitles/src/cli/`) and the
  window's job queue, through the re-export at the crate root; `tools/visual_validation/`
  (`worker_inputs`).
- Rules:
  - a step's record is committed with its outputs, and removed before the step runs again, so a
    killed job resumes from the last finished step (the module header);
  - `--rerun` clears a step and every step that reads it, their per-frame rows with the steps
    that write them, in one transaction, and keeps the owner's corrections
    (`a_rerun_clears_its_steps_and_their_dependents_in_one_transaction_and_puts_the_record`,
    `per_frame_rows_stay_when_no_step_that_writes_them_is_cleared` in `tests/rerun.rs`), and a
    cleared localized video keeps the video it wrote as its earlier one
    (`a_cleared_localized_video_keeps_the_video_it_wrote_as_its_earlier_one`);
  - a step about to run loses its record and the rows of its per-frame table alone
    (`forgetting_a_step_removes_its_record_and_keeps_its_documents`,
    `forgetting_a_step_that_owns_a_per_frame_table_clears_its_rows_alone`);
  - one worker loads the GPU at a time; the shot scan, which runs beside it, loads none;
  - a step starts only after every step it reads has finished, and none starts once the job is
    cancelled;
  - the shot scan's thread stays alive until its worker is reaped, since a child dies with the
    thread that started it (`crates/child_process/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow the runner walks.
- [System overview](/documentation/architecture/system_overview.md#processes) — the processes a job
  starts.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#resume-and-reruns) —
  the one-transaction rerun and the orphan cleanup.

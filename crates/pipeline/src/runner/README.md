# Step runner

`run_job`: one video through every step, in the order `StepName::ALL` gives, skipping what is still
valid, running the shot scan beside the audio steps and the visual lane beside adjudication,
committing each step's outputs with its record, and writing the report at the end.

## Contents

```text
crates/pipeline/src/runner/
├── lane.rs   the visual lane: where the main walk starts and joins it, and its thread
├── mod.rs    `run_job`, `JobOptions`, `JobOutcome`, the CUDA environment, `worker_inputs`
├── rerun.rs  the job record and `--rerun` cleared in one transaction; a record forgotten to rerun
├── walk.rs   `Steps`: one step's resume check, run, commit and events, and the job's first failure
└── tests/    unit tests for reruns, a forgotten record, a step's run and failure, and the lane
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

It then walks the steps, checking `JobOptions::cancel` before each. Every step, wherever it runs,
goes through `walk::Steps`: a valid step (`resume::is_valid` over a fresh snapshot) is skipped;
any other has its fingerprint taken, its record and the rows of the per-frame table it owns
removed (`rerun::forget`), and runs through `tasks::in_process` on a `StepIo` of the store or
through `workers::run_worker` with the stored values `worker_inputs` gives it (every
`graph::reads` value, less the optional ones the job lacks), as `graph::placement` says. Either
way the outputs the step wrote come back as an uncommitted `StepWrite`, and `stamp` commits them
with its `StepRecord` (fingerprint, finish time, measure) in one transaction; a step that wrote
none still commits its record. A worker gets the cancel token, which its watchdog watches, and a
GPU worker first takes the machine-wide lock `JobOptions::gpu_lock`, which also keeps the GPU
steps of the lane and the main walk apart.

Two threads run beside the main walk, each in a `std::thread::scope`. The shot scan starts on
the main walk, then runs and commits on a thread of its own. The visual lane (`lane.rs`,
`graph::VISUAL_LANE`) walks `text_detect`, `text_read` and `text_track` in order on another:
`lane::plan` tells the main walk, at each step, to join the shot scan (before the lane starts and
before any step that reads the scan), to start the lane (at `graph::VISUAL_LANE_STARTS_AT`,
`adjudicate`, whether that step runs or is skipped), to join the lane (before the first main
step that reads one of its steps, `text_translate`), and to leave the lane's steps to it. Each
lane step runs in a `step` span made on the runner's thread as a child of the caller's job span,
so its lines group under the job. Threads still running when the walk ends are joined there; the
steps run and skipped by every thread are merged in run order.

A step that fails reports `Progress::StepFailed`, becomes the job's first failure when it is the
first, and sets the cancel token, so the other threads' workers are killed and no further step
starts; a step stopped only by that cancel reports nothing, and the walk then ends with the first
failure rather than "cancelled". A step the owner cancels reports `StepFailed` as cancelled. When
the walk ends with an error the runner sets the cancel token, so the scan and the lane stop
instead of running to their end. The CUDA environment, found once through
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
  and programs' lines, the shot scan's thread and the lane's steps log inside it), its debug lines (the job,
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
  - one worker loads the GPU at a time; the shot scan, which runs beside it, loads none, and the
    lane's GPU steps take the lock as the main walk's do;
  - a step starts only after every step it reads has finished, and none starts once the job is
    cancelled: the lane starts at adjudication after the shot scan is joined and is joined
    before translation, and the main walk runs none of its steps
    (`the_lane_starts_at_adjudication_and_is_joined_before_translation`,
    `the_main_walk_runs_every_step_but_the_lanes`,
    `the_shot_scan_is_joined_before_the_lane_starts_and_before_its_readers`,
    `every_main_step_that_reads_the_lane_comes_after_its_join` in `tests/lane.rs`);
  - the lane runs its steps in order and stops the lane and the job at its first failure
    (`the_lane_runs_its_steps_in_order_on_its_thread`,
    `a_failed_lane_step_stops_the_lane_and_the_job`);
  - a failed step reports once, stops the job and is its error; a step stopped by another's
    failure reports nothing; an owner's stop is reported
    (`a_failed_step_reports_once_stops_the_job_and_is_its_failure`,
    `a_step_stopped_by_another_steps_failure_reports_nothing`,
    `a_step_the_owner_stopped_reports_the_stop`,
    `a_walk_stopped_by_another_steps_failure_ends_with_that_failure` in `tests/walk.rs`);
  - the shot scan's and the lane's threads stay alive until their workers are reaped, since a
    child dies with the thread that started it (`crates/child_process/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stage flow the runner walks.
- [System overview](/documentation/architecture/system_overview.md#processes) — the processes a job
  starts.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#resume-and-reruns) —
  the one-transaction rerun and the orphan cleanup.

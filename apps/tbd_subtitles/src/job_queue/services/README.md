# Job queue services

The queue logic, with no rendering code: editing the queue, the thread that runs its jobs,
following their progress, the time left, and the queue kept across windows.

## Contents

```text
apps/tbd_subtitles/src/job_queue/services/
├── job_runner.rs         the long-lived thread that runs one job at a time and reports its events
├── mod.rs                the module list
├── progress_tracking.rs  a runner event folded into the running job's progress
├── queue_editing.rs      add videos or folders, remove, move, retry, and the next waiting job
├── queue_store.rs        `queue.json`: the queue written after each change and read at start
├── tests/                unit tests for each file here
└── time_left.rs          step rates from earlier jobs or the pilot, and a job's time left
```

## How it works

`queue_editing::add_videos` queues each video not already waiting or running; a folder stands for
its videos with no subtitle file beside them. `next_waiting(queue, kind)` names the job that runs
next in a lane: full runs in queue order, review runs in theirs; `queue_review` queues a video's
review run unless one already waits. `job_runner::start` spawns one thread that runs each
`Command` it is handed with the given `RunJob` (the pipeline's `run_job`, or a stand-in in the
tests) and sends every progress event and the outcome back, waking the window; the thread lives as
long as the window, so the workers it starts are not killed early. `progress_tracking::apply`
moves each step row as the events arrive. `time_left::from_history` reads every job's `job.json`
and `probe.json` in the work folder for each step's mean seconds per second of video, over the
pilot's rates; `estimate` sums the steps still to run, leaving out the shot scan that runs beside
them. `queue_store` keeps each job's video, kind and coarse state.

## Boundaries

- Depends on: `crate::job_queue::models`; `crate::core::background::Wake`; `pipeline`;
  `job_model`; `serde` and `serde_json`.
- Used by: `crate::application` (`actions::queue`, `mod.rs`); `crate::job_queue::ui`
  (`time_left::estimate`).
- Rules:
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - review runs wait in their own lane and are queued once per video
    (`review_runs_wait_in_their_own_lane_and_are_queued_once` in `tests/queue_editing.rs`);
  - jobs run in order and a cancelled job ends cancelled
    (`jobs_run_in_order_and_report_each_event`, `a_cancelled_job_ends_cancelled` in
    `tests/job_runner.rs`);
  - the shot scan never adds to the time left, and a step keeps its own pace
    (`done_skipped_and_the_shot_scan_add_nothing_and_a_step_keeps_its_own_pace` in
    `tests/time_left.rs`);
  - a job running when the window closed waits again
    (`a_saved_queue_loads_back_with_the_running_job_waiting` in `tests/queue_store.rs`).

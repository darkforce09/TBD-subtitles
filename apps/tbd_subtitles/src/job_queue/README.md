# Job queue

The feature that runs the videos' jobs: the queue panel on the left of the window, the selected
job's progress on the right, the edits of the queue, the thread that runs one job at a time, the
time left, and the queue kept across windows.

## Contents

```text
apps/tbd_subtitles/src/job_queue/
├── events.rs  `JobQueueEvent`: add, select, remove, move, cancel, retry, start and pause
├── mod.rs     the module tree
├── models/    the queue and its jobs, a running job's progress, the step rates, the borrowed view
├── services/  queue edits, the job runner, progress, time left and the kept queue
└── ui/        the queue panel and the selected job's progress
```

## How it works

```text
queue panel ──▶ JobQueueEvent ──▶ application (actions::queue, actions::runner)
                                    ├── queue_editing: add, remove, move, retry, next waiting
                                    ├── Start: next waiting full run + JobOptions ──▶ job_runner
                                    └── Cancel: the job's CancelToken
line review Save ──▶ queue_editing::queue_review ──▶ next waiting review run ──▶ review job_runner
either runner's thread ──run_job──▶ RunnerEvent::Progress / Ended ──▶ progress_tracking ──▶ queue
                                                                  └──▶ queue_store (queue.json)
```

A full run waits until the owner presses Start; the queue then runs the waiting full runs in
order, one at a time, on its runner's long-lived thread, and stops when none is left or the owner
pauses it. A review run, queued when the owner saves a correction and counting each correction
saved while it waits, runs at once on a second runner, unless a full run of the same video is
running; only its review step and the steps after it run. Both lanes live in one process, so the
job lock does not keep them apart: a full run whose video has a review run running waits, and the
full lane waits with it. No job starts while a model or runtime archive is missing. A job takes
the settings saved now until it first starts; from then on it keeps its own, the ones in its
`job.json`, as a review run always does, and `queue.json` remembers that across windows. The
steps a job is to run again go to the pipeline as `JobOptions.rerun`, and leave the queue once the
pipeline has recorded them (its first event); a job that fails before that keeps them. Cancel sets
the running job's token: the pipeline kills its worker and the job ends cancelled with the number
of finished steps it kept, so Retry resumes after them; a failed job records the step that failed
(its stage and plain title shown), the message and the steps it kept. The runner's events move
each step from pending to running (with its progress and last line) to done or failed; the time
left is each step's measured seconds per second of video, from the earlier jobs in the work folder
or the pilot's, over the steps still to run, a running step judged by its own pace once it reports
one. The queue is written to `queue.json` after every change: a job running when the window closed
waits again next time.

## Public surface

- `models::{queue, progress, view}`, `events::JobQueueEvent`, `ui::{queue_panel_ui,
  progress_view_ui}` and the services, for the application.

## Boundaries

- Depends on: `pipeline` (`run_job`, `JobOptions`, `Progress`, `CancelToken`), `job_model`,
  `crate::core` (`background`, `format`, `steps`, `ui`), `serde` and `serde_json`; `eframe::egui`
  in `ui/` only.
- Used by: `crate::application` (`actions::{queue, review, runner}`, `feature_views`, `window`).
- Rules:
  - `models/` and `services/` never name egui or eframe, and the feature imports neither
    `application` nor `cli` (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - one full run runs at a time, a review run starts at once, and none while a model is missing
    (`started_jobs_run_one_after_another_and_the_queue_is_kept`,
    `a_saved_correction_queues_a_review_run_that_runs_at_once`,
    `no_job_starts_while_a_model_is_missing` in `apps/tbd_subtitles/src/application/tests/rendering.rs`);
  - review runs wait in their own lane and one video has at most one waiting
    (`review_runs_wait_in_their_own_lane_and_are_queued_once` in `services/tests/queue_editing.rs`);
  - a running job is never removed, and only waiting jobs move
    (`a_running_job_cannot_be_removed`, `waiting_jobs_move_among_themselves` in
    `services/tests/queue_editing.rs`);
  - a full run waits while a review run of its video runs
    (`a_full_run_waits_while_its_videos_review_run_runs` in
    `apps/tbd_subtitles/src/application/tests/rendering.rs`);
  - a failed job records its step and the steps it kept, a cancelled job its kept steps, and a
    started job keeps its own settings and hands its steps to run again to the pipeline, keeping
    them when it fails before the pipeline records them
    (`a_failed_job_records_its_step_and_the_steps_it_kept`,
    `a_cancelled_job_keeps_its_finished_steps_and_can_be_retried`,
    `a_job_failing_before_it_starts_keeps_its_steps_to_run_again` in
    `apps/tbd_subtitles/src/application/tests/rendering.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue and progress behaviour.

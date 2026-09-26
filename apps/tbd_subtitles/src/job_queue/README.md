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
queue panel ──▶ JobQueueEvent ──▶ application (actions::queue)
                                    ├── queue_editing: add, remove, move, retry, next waiting
                                    ├── Start: next waiting job + its JobOptions ──▶ job_runner
                                    └── Cancel: the job's CancelToken
job_runner thread ──run_job──▶ RunnerEvent::Progress / Ended ──▶ progress_tracking ──▶ queue
                                                             └──▶ queue_store (queue.json)
```

A job waits until the owner presses Start; the queue then runs the waiting jobs in order, one at a
time, on the runner's long-lived thread, and stops when none is left or the owner pauses it. No
job starts while a model or runtime archive is missing. A new job takes the settings saved now; a
retry and a review run keep the settings in their own `job.json`. Cancel sets the running job's
token: the pipeline kills its worker and the job ends cancelled, its finished steps kept, so Retry
resumes after them. The runner's events move each step from pending to running (with its
progress and last line) to done or failed; the time left is each step's measured seconds per
second of video, from the earlier jobs in the work folder or the pilot's, over the steps still to
run, a running step judged by its own pace once it reports one. The queue is written to
`queue.json` after every change: a job running when the window closed waits again next time.

## Public surface

- `models::{queue, progress, view}`, `events::JobQueueEvent`, `ui::{queue_panel_ui,
  progress_view_ui}` and the services, for the application.

## Boundaries

- Depends on: `pipeline` (`run_job`, `JobOptions`, `Progress`, `CancelToken`), `job_model`,
  `crate::core` (`background`, `format`, `ui`), `serde` and `serde_json`; `eframe::egui` in `ui/`
  only.
- Used by: `crate::application` (`actions::queue`, `feature_views`, `window`).
- Rules:
  - `models/` and `services/` never name egui or eframe, and the feature imports neither
    `application` nor `cli` (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - one job runs at a time, and none while a model is missing
    (`started_jobs_run_one_after_another_and_the_queue_is_kept`,
    `no_job_starts_while_a_model_is_missing` in `apps/tbd_subtitles/src/application/tests/rendering.rs`);
  - a running job is never removed, and only waiting jobs move
    (`a_running_job_cannot_be_removed`, `waiting_jobs_move_among_themselves` in
    `services/tests/queue_editing.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue and progress behaviour.

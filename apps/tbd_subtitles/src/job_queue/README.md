# Job queue

The feature that runs the videos' jobs: the toolbar across the top of the window, the sidebar of
videos on the left, the empty list's card and the drop overlay, the selected job's cards,
progress and stages on the right, the edits of the queue, the thread that runs one job at a time,
the time left, and the queue kept across windows.

## Contents

```text
apps/tbd_subtitles/src/job_queue/
├── events.rs  `JobQueueEvent`: every request of the toolbar, the sidebar and its menus
├── mod.rs     the module tree
├── models/    the queue and its jobs, the sidebar's rows, a job's progress, the borrowed view
├── services/  edits, sidebar rows and status lines, the runner, progress, stages, time left, queue.json
└── ui/        the toolbar, the sidebar, its rows and menus, the empty card, the drop overlay, job cards
```

## How it works

```text
toolbar, sidebar, row menu ──▶ JobQueueEvent ──▶ application (actions::queue, actions::runner)
                                    ├── queue_editing: add, remove/restore, move, try again, run again
                                    ├── Start: next waiting full run + JobOptions ──▶ job_runner
                                    └── Cancel: the job's CancelToken
line review Save ──▶ queue_editing::queue_review ──▶ next waiting review run ──▶ review job_runner
either runner's thread ──run_job──▶ RunnerEvent::Progress / Ended ──▶ progress_tracking ──▶ queue
                                                                  └──▶ queue_store (queue.json)
queue ──▶ sidebar_rows (one row per video) ──▶ status_text ──▶ sidebar
```

The toolbar holds Add Videos… and Add Folder…, one queue button and the gear that opens Settings.
The button is Start Queue while the queue is off (disabled, with the reason beside it, when no full
run waits or a model is missing), Pause After This Video while it runs, and Resume Queue once
paused while a full run still runs; it counts full runs only, since correction runs start by
themselves. Pressing Start runs the waiting full runs in order, one at a time, on their runner's
long-lived thread, and the queue stops when none is left or after the running video once paused.
A review run (a correction run), queued when the owner saves a correction and counting each
correction saved while it waits, runs at once on a second runner, unless a full run of the same
video is running; only its review step and the steps after it run. Both lanes live in one process,
so the job lock does not keep them apart: a full run whose video has a review run running waits,
and the full lane waits with it. No job starts while a model or runtime archive is missing.

The sidebar shows one row per video in the sections Now, Up Next and Done (the newest ended job
first), each under a heading with its count that folds it away. A correction run folds into its
video's row, which then reads "Updating subtitles · 2 corrections" and offers Stop Updating
Subtitles while it runs; one that failed or was cancelled keeps a row of its own, so it can be
tried again. Each row has a status mark and a status line ("Settling the words · about
4 min left", "Waiting · 2nd in line", "Failed at Hear the speech", "Cancelled · 9 finished steps
kept"); a click anywhere selects it, a red round ✕ on hover removes it, a waiting row drags to
another place in line with a line showing where it lands, and a right click opens the menu of its
state: Run Next, Move Up, Move Down; Cancel or Stop Updating Subtitles; Check Lines, Open in
Player, Show in Folder, Copy Subtitle Path, Run Again with Current Settings; Try Again; Remove from
List. A removed row keeps its files on disk and comes back where it was with the Undo of its toast;
a newer removal takes the older Undo away. With no video the sidebar says how to add
some and the right side shows a card with the add buttons; while files are dragged over the
window, an overlay says to drop them.

A job takes the settings saved now until it first starts; from then on it keeps its own, the ones
in its `job.json`, as a review run always does, and `queue.json` remembers that across windows.
Try Again puts a failed or cancelled job first in line, resuming after the steps it kept, and Run
Again puts a finished one first in line with the settings saved now, so only the steps they change
run again; either starts at once when its lane is idle, without turning the queue on, else it
waits first in line (for Start Queue while the queue is off), and Run Again with the settings the
job ran with runs nothing and says so. Neither Undo, Try Again nor Run Again puts a video back
while another run of it waits or runs: the window says it is already in the list. The steps a job is to run again
go to the pipeline as `JobOptions.rerun`, and leave the queue once the pipeline has recorded them
(its first event); a job that fails before that keeps them. Cancel sets the running job's token:
the pipeline kills its worker and the job ends cancelled with the number of finished steps it
kept; a failed job records the step that failed (its stage and plain title shown), the message and
the steps it kept. The runner's events move each step from pending to running (with its progress
and last line) to done or failed; the time left is each step's measured seconds per second of
video, from the earlier jobs in the work folder or the pilot's, over the steps still to run, a
running step judged by its own pace once it reports one. The queue is written to `queue.json`
after every change: a job running when the window closed waits again next time.

## Public surface

- `models::{queue, progress, sidebar, view}`, `events::JobQueueEvent`, `ui::{toolbar_ui,
  sidebar_ui, row_order, empty_state_ui, drop_overlay_ui, progress_view_ui}` and the services, for
  the application.

## Boundaries

- Depends on: `pipeline` (`run_job`, `JobOptions`, `Progress`, `CancelToken`), `job_model`,
  `crate::core` (`background`, `format`, `steps`, `ui`), `serde` and `serde_json`; `eframe::egui`
  in `ui/` only.
- Used by: `crate::application` (`actions::{queue, review, runner}`, `feature_views`, `window`,
  `shortcuts`).
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
  - a row whose job or correction run runs is never removed, a removed row comes back where it
    was unless its video is queued again, and only waiting full runs move
    (`a_running_job_cannot_be_removed`, `a_video_is_not_put_back_while_another_run_of_it_waits`,
    `a_row_whose_correction_run_runs_stays`,
    `a_removed_row_goes_back_where_it_was_with_its_correction_runs`,
    `waiting_jobs_move_among_themselves`, `a_dragged_job_lands_before_its_target_or_last` in
    `services/tests/queue_editing.rs`);
  - the queue button counts only full runs, and Pause and Resume stay while a model is missing
    (`the_queue_control_counts_only_full_runs`,
    `pause_and_resume_stay_reachable_while_a_model_is_missing` in
    `services/tests/queue_editing.rs`); a correction run shows on its video's row, and the newest
    ended job comes first (`correction_runs_fold_into_their_videos_row`,
    `done_rows_list_the_newest_ended_first` in `services/tests/sidebar_rows.rs`);
  - a full run waits while a review run of its video runs
    (`a_full_run_waits_while_its_videos_review_run_runs` in
    `apps/tbd_subtitles/src/application/tests/rendering.rs`);
  - a failed job records its step and the steps it kept, a cancelled job its kept steps, and a
    job tried again starts at once without turning the queue on, keeping its own settings
    (`a_failed_job_records_its_step_and_the_steps_it_kept`,
    `a_cancelled_job_keeps_its_finished_steps_and_can_be_retried`,
    `a_job_failing_before_it_starts_keeps_its_steps_to_run_again` in
    `apps/tbd_subtitles/src/application/tests/rendering.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue and progress behaviour.

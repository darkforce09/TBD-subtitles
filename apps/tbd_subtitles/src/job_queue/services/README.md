# Job queue services

The queue logic, with no rendering code: editing the queue, the sidebar's rows and their status
lines, the thread that runs its jobs, following their progress, the time left, and the queue kept
across windows.

## Contents

```text
apps/tbd_subtitles/src/job_queue/services/
├── job_runner.rs         the long-lived thread that runs one job at a time and reports its events
├── mod.rs                the module list
├── progress_tracking.rs  a runner event folded into the running job's progress
├── queue_editing.rs      add, remove and restore, move, try and run again, the next job, the button
├── queue_store.rs        `queue.json`: the queue written after each change and read at start
├── sidebar_rows.rs       one row per video in Now, Up Next and Done, correction runs folded in
├── status_text.rs        a row's status line: "Waiting · 2nd in line", "Failed at Hear the speech"
├── tests/                unit tests for each file here
└── time_left.rs          step rates from earlier jobs or the pilot, and a job's time left
```

## How it works

`queue_editing::add_videos` queues each video not already waiting or running; a folder stands for
its videos with no subtitle file beside them (`subtitle_file` finds one in any format the app
writes). `remove` takes a row out with the correction runs folded into it, unless something on it
runs, and hands the selection to the row now in its place or the one before; `restore` puts the
`Removed` back at the indices it had and selects it, unless a run of the same kind of its video
waits or runs meanwhile (`Refusal::AlreadyQueued`). `move_before` moves a waiting full run to
just before another or after the last, and `move_job` moves it up, down or first among the waiting
full runs. `try_again` puts an ended job back to waiting first in line among the runs of its kind,
with a step to run again when one is named; `run_again` does the same with the settings saved now;
both refuse as `restore` does. `newest_ended_first` moves a job that just ended ahead of every
other ended job, so the Done section lists the newest first and `queue.json` keeps that order.
`queue_control` chooses the toolbar's button from the full runs only: Pause After This Video while
the queue runs, Resume Queue ("Pauses after Dressrosa 16") once paused with a full run running,
both even while a model is missing, else Start Queue (with the reason it is off: "Download the
models first", "Nothing is waiting", "Add videos to start"). `next_waiting(queue, kind)` names the
job that runs next in a lane: full runs in queue order, review runs in theirs; `queue_review`
queues a video's review run carrying one correction, or adds the correction to the one that
already waits.

`sidebar_rows::rows` builds the sidebar: a correction run that waits, runs or finished folds into
the row of its video's newest finished full run (else its last full run), naming the one running
now; one that failed or was cancelled keeps a row of its own; rows come in the order Now, Up Next,
Done, each in queue order (ended jobs stand newest first),
and the waiting full runs are numbered by their place in line. `status_text::status` writes a
row's line from its job: a running job's stage and time left ("Settling the words · about 4 min
left", "Stopping…"), a waiting job's place, shown past the first only while the queue runs, where
a failed job failed, the steps a cancelled one kept, "Updating subtitles · 2 corrections" while a
correction run is pending, "Needs attention · 1 problem" for a job that does not pass the quality
check, else "Subtitles ready".
`job_runner::start` spawns one thread that runs each `Command` it is handed with the given
`RunJob` (the pipeline's `run_job`, or a stand-in in the tests) and sends every progress event and
the outcome back, waking the window; the thread lives as long as the window, so the workers it
starts are not killed early. `progress_tracking::apply` moves each step row as the events arrive.
`time_left::from_history` reads every job's `job.json` and `probe.json` in the work folder for
each step's mean seconds per second of video, over the pilot's rates; `estimate` sums the steps
still to run, leaving out the shot scan that runs beside them. `queue_store` keeps each job's
video, kind and coarse state, whether it keeps its own settings, the steps it runs again, its
corrections, and where it failed with the finished steps it kept; each of those fields has a
default, so a file written before it existed still loads, a job there that no longer waits
keeping its own settings.

## Boundaries

- Depends on: `crate::job_queue::models`; `crate::core::{background::Wake, format, steps}`;
  `pipeline`; `job_model`; `serde` and `serde_json`.
- Used by: `crate::application` (`actions::{queue, review, runner}`, `mod.rs`);
  `crate::job_queue::ui` (`time_left::estimate`, `queue_editing::{queue_control, subtitle_file}`,
  `sidebar_rows`, `status_text`).
- Rules:
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - review runs wait in their own lane and are queued once per video
    (`review_runs_wait_in_their_own_lane_and_are_queued_once` in `tests/queue_editing.rs`);
  - a removed row comes back where it was, a row whose run runs stays, a dragged job lands before
    its target, a job tried again goes first in line, and the queue's button counts full runs only
    (`a_removed_row_goes_back_where_it_was_with_its_correction_runs`,
    `a_row_whose_correction_run_runs_stays`, `a_dragged_job_lands_before_its_target_or_last`,
    `trying_again_puts_an_ended_job_first_in_line`, `the_queue_control_counts_only_full_runs`,
    `pause_and_resume_stay_reachable_while_a_model_is_missing`,
    `a_video_is_not_put_back_while_another_run_of_it_waits`, `ended_jobs_stand_newest_first` in
    `tests/queue_editing.rs`);
  - every job is on one row, a correction run on its video's (`correction_runs_fold_into_their_videos_row`,
    `a_failed_or_lone_correction_run_keeps_its_own_row` in `tests/sidebar_rows.rs`), and a place in
    line past the first shows only while the queue runs
    (`waiting_rows_show_their_place_only_while_the_queue_runs` in `tests/status_text.rs`);
  - jobs run in order and a cancelled job ends cancelled
    (`jobs_run_in_order_and_report_each_event`, `a_cancelled_job_ends_cancelled` in
    `tests/job_runner.rs`);
  - the shot scan never adds to the time left, and a step keeps its own pace
    (`done_skipped_and_the_shot_scan_add_nothing_and_a_step_keeps_its_own_pace` in
    `tests/time_left.rs`);
  - a job running when the window closed waits again, and a file without the newer fields loads
    (`a_saved_queue_loads_back_with_the_running_job_waiting`,
    `a_file_written_before_the_new_fields_still_loads` in `tests/queue_store.rs`).

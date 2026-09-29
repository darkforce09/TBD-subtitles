# Job queue services

The queue logic, with no rendering code: editing the queue, the sidebar's rows and their status
lines, the detail pane's words for the selected job, the threads that run its jobs, following
their progress, a job's six stages, the time left, the queue kept across windows, the videos in
a folder, the watch folders' scans, every video ever queued, and the notice when a job ends.

## Contents

```text
apps/tbd_subtitles/src/job_queue/services/
├── folder_watcher.rs     the thread that scans the watch folders and sends each complete video
├── job_notice.rs         the desktop notice's words for a full run that finished or failed
├── job_runner.rs         the long-lived thread that runs one job at a time and reports its events
├── mod.rs                the module list
├── progress_log.rs       a job's events and end as log lines, a step's advance once per tenth
├── progress_tracking.rs  a runner event folded into the running job's progress
├── queue_editing.rs      add, remove and restore, move, try and run again, the next job, the button
├── queue_store.rs        `queue.json`: the queue written after each change and read at start
├── queued_history.rs     `queued_videos.json`: every video ever queued, so none is queued twice
├── review_lanes.rs       four runners for correction runs, each holding one run and its cancel token
├── sidebar_rows.rs       one row per video in Now, Up Next and Done, correction runs folded in
├── stage_progress.rs     six stage rows, each with its steps, from the step states or the failure
├── status_text.rs        a row's status line, the detail pane's line, when a job starts
├── tests/                unit tests for each file here
├── time_left.rs          step rates from earlier jobs or the pilot, and a job's time left
├── video_files.rs        what is a video, its subtitle file, part files, videos in or under a folder
└── watch_scan.rs         one scan of the watch folders: videos whose size and time stopped changing
```

## How it works

`queue_editing::add_videos` queues each video not already waiting or running; a folder stands for
its videos directly in it with no subtitle file beside them (`video_files::videos_in_folder`;
`video_files::subtitle_file` finds one in any format the app writes). `remove` takes a row out with the correction runs folded into it, unless something on it
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
job that runs next in a lane: full runs in queue order, review runs in theirs;
`first_startable(queue, kind, startable)` the first waiting one the application lets start (the
review lanes skip a video another run or Fix It holds); `queue_review` queues a video's review run
carrying the corrections it is given (one per save, or every line a Fix It run changed), or adds
them to the one that already waits.

`sidebar_rows::rows` builds the sidebar: a correction run that waits, runs or finished folds into
the row of its video's newest finished full run (else its last full run), naming the one running
now; one that failed or was cancelled keeps a row of its own; rows come in the order Now, Up Next,
Done, each in queue order (ended jobs stand newest first),
and the waiting full runs are numbered by their place in line. `status_text::status` writes a
row's line from its job: a running job's stage and time left ("Settling the words · about 4 min
left", "Stopping…"), a waiting job's place, shown past the first only while the queue runs, where
a failed job failed, the steps a cancelled one kept; for a finished job, "Fixing with Claude · 2
of 4" while Fix It fixes its video (its step of four, the correction run of its changes the
fourth), else "Updating subtitles · 2 corrections" while a correction run is pending; for a
finished job whose files the application read, its summary's "Needs attention · 1 problem" when
it does not pass the quality check, else "Subtitles ready · 38 to check", "Subtitles ready ·
fixed by Claude" once none is left to check and a Fix It run of the job as it stands answered
lines, "Subtitles ready · all checked" once every line worth a listen is checked, and "Subtitles
ready" when none is; without a summary, "Needs attention" from the run's own result, else
"Subtitles ready". `status_text::fails_the_check` says the same verdict for the
row's warning mark. `status_text::detail_line` writes the line under the detail pane's
title for a job that has not finished: "25:59 video · running for 10 min 00 s" ("Running for 3 s"
until the length is known), "Length known once it starts · 2nd in line", or the row's line of a
failed or cancelled job; `place_in_line` counts a waiting job's place among the waiting runs of its
kind; `waiting_start` says when a waiting job starts (a correction run as soon as its video is
free; a full run after the current video, after the videos before it, or on Start Queue; either
once the models are on disk), and `try_again_start` when an ended
job tried again now would: at once when its lane is idle (for a correction run, fewer than four
run) and its video runs nothing else, next
while the queue runs (a correction run always), else first in line waiting for Start Queue, and
never before the models are on disk. `stage_progress::running` turns a running job's step states
into the six stages of `core::steps`, each with its steps' lines: kept (skipped, or not stale),
to run, running (its share done from the step's progress, and the seconds since it started), done
(its measured seconds) or failed; a stage is failed when a step failed, running when a step runs
or some are done while others wait (between two of its steps), kept when all are kept, done when
none waits (the sum of its steps' seconds), else to run. `stage_progress::failed` gives a failed
job's stages: the steps before the failed one kept, it failed, the rest to run; `step_number`
numbers a step from 1 of 18.
The shot scan runs in the background until the cues join it: while it runs its line says so, and
neither it nor a shot scan still to start holds "Read the video" open, which is done once the
video's details are read. A failed job's stages come from its `Failure`'s finished steps: those
done in the run that failed are done with their seconds, those skipped as still valid are kept,
the failed step failed and every other step, a shot scan never joined too, to run, so the list
keeps exactly the steps the failure counts.
`job_runner::start` spawns one named thread that runs each `Command` it is handed with the given
`RunJob` (the pipeline's `run_job`, or a stand-in in the tests) and sends every progress event and
the outcome back, waking the window; the thread lives as long as the window, so the workers it
starts are not killed early. `review_lanes::ReviewLanes` starts `REVIEW_LANES` (four) such
threads, "review-runner-1" to "review-runner-4", for correction runs: `run` hands a run to the
first idle lane, which holds it with its cancel token until `release`; `token` finds a run's
token to cancel it, and `drain` gathers every lane's events. `progress_tracking::apply` moves each step row as the events arrive.
The thread runs each job in a `job{video}` span and also logs each event and the end under the
`job` target, with the video and the step as fields: `progress_log::ProgressLog` writes a step's
start, message, finish (its time, RAM, VRAM and notes) or skip as info, its advance at debug once
per tenth, and a failure as an error, never naming the video or step in the words, which the log
window shows above them. A worker's model call (`Progress::ModelCall`) is not sent to the window's
queue: `progress_log::emit_call` logs it as the exchange event the log window keeps.
`time_left::from_history` reads every job's `job.json` and `probe.json` in the work folder for
each step's mean seconds per second of video, over the pilot's rates; `estimate` sums the steps
still to run, leaving out the shot scan that runs beside them. `queue_store` keeps each job's
video, kind and coarse state, whether it keeps its own settings, the steps it runs again, its
corrections, and where it failed with the steps it had finished (each with its seconds, or still
valid); each of those fields has a default, so a file written before it existed still loads, a
job there that no longer waits keeping its own settings. `read_finished_steps` runs when the
window opens: a failure an older file kept, which knows only how many steps it kept, gets the
steps before its failed one that its job's `job.json` records, with their seconds, else the first
of them it kept, done in a time not known, and then keeps as many as it lists.

`video_files::is_video` knows a video by its extension (`VIDEO_EXTENSIONS`, any case), and
`has_partial_sibling` by a downloader's part file beside it ("<file name>.part", ".crdownload" or
".!qB"). `videos_under` walks a folder and its subfolders down to `MAX_DEPTH` (16) folders below
it, sorted, leaving out hidden entries, symlinked folders, empty files, videos still downloading
and videos with a subtitle file; a folder it cannot read is skipped with a debug line.
`watch_scan::step` folds one scan's videos, each with its `Sample` (size and modification time),
into a `WatchScan`: a video whose sample equals the one the scan before saw has stopped changing
and is reported, and never again by the same `WatchScan`, even after it vanishes and comes back;
samples of videos no longer seen are dropped, so a video not yet reported that comes back is a
first sighting again.
`WatchScan::scan` samples every video under the watch folders and hands them to `step`; a missing
folder is warned about once until it reappears. `folder_watcher::start` spawns the thread
"folder-watcher": it scans at once and then every `interval` (`WATCH_INTERVAL`, 15 s, in the
app), and at once when `set_folders` hands it a different list; each non-empty list of complete
videos goes to `found` and wakes the window; an empty folder list scans nothing, and the thread
ends when the `FolderWatcher` is dropped. `queued_history::QueuedHistory` is every video the app
ever queued, from any source (`record`, `record_queue`), so a watch folder never queues a video
twice, even one that failed or was removed; `load` reads `queued_videos.json` in the app data
folder (`default_path`), a missing or broken file an empty history, and `save` writes it through a
part file. `job_notice::ended_notice` words the desktop notice for a full run that has just ended:
"Subtitles ready: <video>" with "The quality check passed." or "Quality check: <problems>; 12
lines to check.", or "<video> failed" with "At <step title>: <message>", cut to 200 characters;
a correction run, or a job waiting, running, cancelled or finished in an earlier window, gives
none.

## Boundaries

- Depends on: `crate::job_queue::models`; `crate::core::{background::Wake, format, steps}`;
  `crate::job_report::models::summary::RowSummary` and `crate::job_report::models::fixing::FIX_STEPS`
  in `status_text.rs`; `pipeline`; `job_model`; `inference::model_store::app_data_dir` and
  `anyhow` in `queued_history.rs`;
  `serde`, `serde_json` and `tracing` (`progress_log`, `job_runner`, `folder_watcher`,
  `watch_scan`, `video_files`, `queued_history`).
- Used by: `crate::application` (`actions::{queue, review, runner}`, `mod.rs`, `detail_view`);
  `crate::job_queue::ui` (`time_left::estimate`, `queue_editing::queue_control`,
  `video_files::subtitle_file`, `sidebar_rows`, `status_text`, `stage_progress`).
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
  - a job has six stages whose steps are every step in order, a stage before the running one is
    done in the sum of its steps, a step this run does not do is kept, a stage between two of its
    steps still runs, a failed step fails its stage, a failed job's list keeps exactly the steps
    it counts, and the shot scan never holds its stage open (`a_running_job_has_six_stages_whose_steps_are_every_step_in_order`,
    `stages_before_the_running_one_are_done_in_the_sum_of_their_steps`,
    `steps_this_run_does_not_do_are_kept_and_never_to_run`,
    `a_stage_between_two_of_its_steps_is_still_running`, `a_failed_step_fails_its_stage`,
    `a_failed_job_lists_exactly_the_steps_it_kept`,
    `the_shot_scan_runs_in_the_background_and_never_holds_its_stage_open`,
    `steps_are_numbered_from_one_of_eighteen` in `tests/stage_progress.rs`);
  - every job is on one row, a correction run on its video's (`correction_runs_fold_into_their_videos_row`,
    `a_failed_or_lone_correction_run_keeps_its_own_row` in `tests/sidebar_rows.rs`), and a place in
    line past the first shows only while the queue runs
    (`waiting_rows_show_their_place_only_while_the_queue_runs` in `tests/status_text.rs`); a
    finished row gives its verdict and lines to check from its summary
    (`finished_rows_give_their_verdict_and_lines_to_check_from_their_files`), says it was fixed
    by Claude only while it passes with nothing to check, and gives Fix It's step while Fix It
    fixes its video (`a_row_fixed_by_claude_says_so_and_a_row_being_fixed_gives_its_step`); the
    detail pane gives a running job's length and a waiting job's place, a waiting job says when it
    starts, and a job tried again starts at once only when its lane and video are idle
    (`the_detail_line_gives_a_running_jobs_length_and_a_waiting_jobs_place`,
    `a_waiting_job_says_when_it_starts`,
    `a_job_tried_again_starts_at_once_only_when_its_lane_and_video_are_idle`,
    `a_correction_run_tried_again_starts_at_once_while_a_review_lane_is_idle` in
    `tests/status_text.rs`);
  - jobs run in order and a cancelled job ends cancelled
    (`jobs_run_in_order_and_report_each_event`, `a_cancelled_job_ends_cancelled` in
    `tests/job_runner.rs`);
  - four review lanes hold one run each, a fifth is refused, a released lane takes a new run,
    and a run's token is its own (`four_lanes_fill_and_a_fifth_run_is_refused`,
    `releasing_a_run_frees_its_lane`, `a_runs_token_is_its_own` in `tests/review_lanes.rs`);
  - the shot scan never adds to the time left, and a step keeps its own pace
    (`done_skipped_and_the_shot_scan_add_nothing_and_a_step_keeps_its_own_pace` in
    `tests/time_left.rs`);
  - a job running when the window closed waits again, a file without the newer fields loads, and
    a failure an older window kept reads its finished steps from `job.json`
    (`a_saved_queue_loads_back_with_the_running_job_waiting`,
    `a_file_written_before_the_new_fields_still_loads`,
    `a_failure_an_older_window_kept_reads_its_finished_steps_from_job_json` in
    `tests/queue_store.rs`);
  - the walk under a folder skips hidden, symlinked, empty, downloading and subtitled entries and
    stops `MAX_DEPTH` folders down (`the_walk_leaves_out_hidden_empty_downloading_and_subtitled_videos`,
    `the_walk_does_not_follow_a_symlinked_folder`, `the_walk_stops_max_depth_folders_down` in
    `tests/video_files.rs`);
  - a watched video is reported only once it stops changing, and only once
    (`a_video_is_not_reported_on_its_first_sighting`,
    `a_growing_video_waits_until_it_stops_changing`,
    `a_reported_video_that_vanishes_and_comes_back_is_not_reported_again` in
    `tests/watch_scan.rs`), and the watcher's thread ends when it is dropped
    (`dropping_the_watcher_ends_its_thread` in `tests/folder_watcher.rs`);
  - a missing or broken history file is an empty history
    (`a_missing_file_is_an_empty_history`, `a_broken_file_is_an_empty_history` in
    `tests/queued_history.rs`);
  - only a full run that finished or failed gives a notice (`other_jobs_give_no_notice` in
    `tests/job_notice.rs`).

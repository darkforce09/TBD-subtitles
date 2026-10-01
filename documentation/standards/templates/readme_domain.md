**Status:** live

# README template: domain or subsystem

**When to use:** a folder with child folders of its own that is none of the more specific kinds:
a crate's `src/` root, one of the app's feature folders such as
`apps/tbd_subtitles/src/job_queue/`, or a group of modules such as a crate's backend folders. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the domain kind adds Public surface.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there.

````markdown
# <Name of the domain or subsystem, in plain words: no path, no backticks>

<One to three sentences: what the domain or subsystem is responsible for.>

## Contents

```text
<repository path of the folder>/
├── <child folder>/  <what it is for: a lowercase phrase, no closing period>
├── <file>           <what it is for; entries run in case-insensitive name order>
└── mod.rs           <the module tree and what it re-exports>
```

## How it works

<How a call, a frame or a job moves through the children, the main types, and the invariants that
span them. Name each child's part in one clause; the child's own README holds the detail.>

## Public surface

- <module::item>: <what it is, and who outside the folder uses it>

## Boundaries

- Depends on: <the modules and crates the folder uses, read from its imports>
- Used by: <every user outside the folder, found with git grep>
- Rules: <the invariants a change here must keep, and the test or gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

Written from `apps/tbd_subtitles/src/job_queue/`, the application module that uses it, its tests
and the architecture tests in `apps/tbd_subtitles/src/tests/architecture_rules.rs`, and shortened:
the folder's own README.md describes every row state, menu and rule. The sample sits in a fenced
block, so no gate reads it as a README; the folder's own README.md is written from the same code
and may differ.

````markdown
# Job queue

The feature that runs the videos' jobs: the toolbar across the top of the window, the sidebar of
videos on the left, the selected job's cards, progress and stages on the right, the edits of the
queue, the threads that run the jobs, the time left, the watch folders and the queue kept across
windows.

## Contents

```text
apps/tbd_subtitles/src/job_queue/
├── events.rs  `JobQueueEvent`: every request of the toolbar, the sidebar and its menus
├── mod.rs     the module tree
├── models/    the queue and its jobs, the sidebar's rows, a job's progress, the watch folders
├── services/  edits, sidebar rows and status lines, the runners, progress, time left, queue.json
└── ui/        the toolbar, the sidebar, its rows and menus, the empty card, the drop overlay, job cards
```

## How it works

```text
toolbar, sidebar, row menu ──▶ JobQueueEvent ──▶ application (actions::queue, actions::runner)
                                    ├── queue_editing: add, remove/restore, move, try again, run again
                                    ├── Start: next waiting full run + JobOptions ──▶ job_runner
                                    └── Cancel: the job's CancelToken
line review Save ──▶ queue_editing ──▶ next waiting review run ──▶ review_lanes
either runner's thread ──run_job──▶ RunnerEvent::Progress / Ended ──▶ progress_tracking ──▶ queue
                                                                  └──▶ queue_store (queue.json)
queue ──▶ sidebar_rows (one row per video) ──▶ status_text ──▶ sidebar
```

The panels change nothing while a frame is drawn: each click is a `JobQueueEvent` the application
applies after the frame. Pressing Start runs the waiting full runs in order, one at a time, on
their runner's long-lived thread. A correction run, queued when the owner saves a correction, runs
at once on one of four review lanes unless a run of the same video is running. The sidebar shows
one row per video in Now, Up Next and Done, each with a status line built by `status_text`; a
finished row gives its verdict and lines to check from the job's records. The runner's events move
each step from pending to running to done or failed; `time_left` judges what is left from each
step's measured pace. `folder_watcher` scans the watch folders and hands the window each video that
has finished arriving, and the queue is written to `queue.json` after every change.

## Public surface

- `events::JobQueueEvent`: what the toolbar, the sidebar and the row menus ask the application to
  do.
- `models::{queue, progress, sidebar, view, watch}`: the queue the application owns and the views
  it lends the panels.
- `ui::{toolbar_ui, sidebar_ui, row_order, empty_state_ui, drop_overlay_ui, progress_view_ui}`: the
  panels `application/feature_views.rs` draws.
- `services`: the edits, the runners and lanes, progress tracking, the queue file and the watch
  folders, for the application; `services::video_files` also for the `process` subcommand.

## Boundaries

- Depends on: `pipeline` (`run_job`, `JobOptions`, `Progress`, `CancelToken`, `work_dir`),
  `job_model`, `inference` (the app data folder), `crate::core`, `crate::job_report::models`
  (a finished row's verdict and count), `serde` and `serde_json`; `eframe::egui` in `ui/` only.
- Used by: `crate::application` (its actions, `feature_views`, `window`, `shortcuts`); the
  `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs`, which expands folders
  through `services::video_files`.
- Rules:
  - `models/` and `services/` never name egui or eframe, and the feature imports neither
    `application` nor `cli` (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - one full run runs at a time, up to four correction runs of different videos run at once, and
    none while a model is missing (`started_jobs_run_one_after_another_and_the_queue_is_kept`,
    `four_correction_runs_run_at_once_but_never_two_of_one_video`,
    `no_job_starts_while_a_model_is_missing` in
    `apps/tbd_subtitles/src/application/tests/rendering.rs`);
  - a running job is never removed, and only waiting full runs move
    (`a_running_job_cannot_be_removed`, `waiting_jobs_move_among_themselves` in
    `services/tests/queue_editing.rs`);
  - a correction run shows on its video's row (`correction_runs_fold_into_their_videos_row` in
    `services/tests/sidebar_rows.rs`);
  - a failed job records its step and the steps it kept
    (`a_failed_job_records_its_step_and_the_steps_it_kept`, in the application's
    `tests/rendering.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue and progress behaviour.
- [Automation](/documentation/features/automation.md) — the watch folders.
````

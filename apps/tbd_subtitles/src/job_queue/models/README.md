# Job queue models

The data the toolbar, the sidebar and the progress view draw, and what the watch folders' scans
remember, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_queue/models/
├── mod.rs       the module list
├── progress.rs  `JobProgress` with a `StepRow` and `StepState` per step, `FinishedStep`, the step `Rates`
├── queue.rs     `Queue`, `QueueItem`, `JobId`, `JobKind`, `JobState`, `Failure`, `JobResult`, `Move`, `Removed`
├── sidebar.rs   `Section`, `SidebarRow` and `ReviewFold`: the sidebar's rows
├── tests/       unit tests for the progress's kept, finished, current, shown and failed steps
├── view.rs      `JobQueueView`, the queue, the rates, the finished rows' summaries, Fix It and Fix All, the clock
└── watch.rs     `Sample` and `WatchScan`: what the watch folders' scans remember between scans
```

## How it works

A `QueueItem` is one video's job: its id, its kind (a full run, or a review run after the owner's
corrections), its state (waiting, running with its progress, finished with its result, finished
in an earlier window, failed with a `Failure`, a finished result naming the localized video when
the job wrote one, cancelled with the finished steps it kept, or busy while another process owns
its database, with that process's pid when known and since when), whether
it keeps its own settings (once it has started), the steps its next run does again, and, for a
review run, how many corrections it carries. A `Failure` names the step that failed (none when
the job failed before its first step), the message, and the steps it had finished, each a
`FinishedStep`: done in that run with its seconds (or in a time not known, for a failure an older
window kept with no job record to read), or still valid from an earlier run; its count of kept
steps is the list's length once the list is known. A `JobProgress`
holds the work directory, the video's length, the steps the job's settings leave idle (which add
no time left) and a row per step: whether this run does it, and pending, still valid, running (since when, how far, its last line), done (its time) or failed;
it answers which steps are finished and so kept (done, or valid from an earlier run),
which step runs now (beside the shot scan, the later one), which one the window names (the running
step, else the last one started, never the shot scan, which runs in the background) and which
failed. `Rates` are each step's seconds per
second of video.

A `QueueItem` names its video by the file name without the extension, and without a leading group
tag such as "[Muhn Pace] " in messages. The `Queue` knows whether it runs and whether the owner
paused it while a full run still runs. `Removed` is a row taken out of the list, its job first and
then the correction runs folded into its row, each with the index it had, so Undo puts it back
where it was. A `ReviewFold` also names the correction run running now, which the row's menu
can stop. A `SidebarRow` is one video in the sidebar: its job, its name, its `Section` (Now,
Up Next or Done), the correction runs folded into it, a `ReviewFold` while some of them wait or
run (with the corrections they carry), its place in line, and whether it can be removed or
dragged. The `JobQueueView` lent to the panels for one frame also carries each finished job's
`RowSummary` (its problems, lines to check and whether Claude fixed it), which the application
reads from the job's work folder, each video Fix It fixes with its step of four
(`fixing_step`: its three passes, then the correction run of its changes), and how many finished
videos Fix All would start Fix It on (`fix_all`).

A `Sample` is a video's size and modification time at one scan of the watch folders. A
`WatchScan` holds each video's sample from the last scan, the videos already reported as complete
and the watch folders found missing; `job_queue::services::watch_scan` changes it.

## Boundaries

- Depends on: `job_model::StepName`; `crate::job_report::models::summary::RowSummary` in
  `view.rs`; `std`.
- Used by: `crate::job_queue::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a failed step is never counted as kept
  (`a_failed_step_is_named_and_not_kept` in `tests/progress.rs`); between two steps the window
  names the last one started, never the shot scan, which is not finished while it runs
  (`the_step_shown_is_the_last_started_and_never_the_shot_scan` in `tests/progress.rs`).

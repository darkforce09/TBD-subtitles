# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── mod.rs       the module list and the re-exports `application` uses
├── queue.rs     the queue's actions: add, select, remove, move, cancel, retry, start and pause
├── report.rs    the selected finished job's report, read when it is selected or ends
├── review.rs    the line review: open, edit, save and queue a review run, play clips, reload
├── runner.rs    starting the next job of each lane with its options, and the runners' events
└── settings.rs  the settings page as the window opens, its actions, and its threads' answers
```

## How it works

`settings.rs` builds the settings page from the settings file (the defaults, with the reason, when
the file cannot be read) and the models its settings need. Its actions change the draft, save it
through `settings::services::page_editing`, open the desktop's chooser for a path setting, start or
stop the model download, and run the machine checks again. `poll_settings` folds the download's
progress, the checks and the work folder's size into the page; a finished download lists the
models again.

`queue.rs` applies each queue event, writes `queue.json` and lets the runner start what may
start. `runner.rs` keeps two lanes, each with its own runner: full runs start one after another
while the queue runs, and review runs start as soon as one waits. Both lanes run in this one
process, so the job lock does not keep them apart: a review run waits while a full run of its
video runs, and the full lane waits while a review run of its next video runs. A job starts only
when every model is on disk, with options built from the saved settings until it first starts,
and from its own `job.json` after that (a review run always), with the steps it is to run again
as `JobOptions.rerun`; starting marks the job as keeping its settings. The runners' events fold
into the queue: progress into the running job (its first event empties the steps to run again,
which the pipeline has recorded by then, so a job failing before it keeps them), the end into a
finished job, a cancelled one with the finished steps it kept, or a failed one with the step that
failed and the steps it kept, after which the step rates, the report and an open review are read
again and the next job of each lane starts.

`report.rs` reads the selected finished job's report when it is selected or a job ends.
`review.rs` opens the selected job's review at the line a finding names (or its first flagged
line), applies the owner's picks, typing and flags to the draft, and on Save or Take back writes
`review.json` and queues a review run of the video, or adds the correction to the one that
waits. Play starts the clip player on the open line with 0.75 s either side, with the video's
sound or the vocal stem; opening another line, Stop and closing the review stop it.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::job_queue` (events, models,
  services); `crate::job_report::services`; `crate::line_review` (events,
  services); `pipeline` (`JobOptions`, `CancelToken`, `workers::Binaries`); `media_io::preview`
  (`Clip`); `crate::core::portal`; `crate::application` (`TbdSubtitlesApp`, `Environment`,
  `background::Chooser`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and a chooser's answer changes only the draft
  (the header of `settings.rs`); a saved correction always queues one review run, and no run of a
  video starts beside a run of the other kind of the same video (the headers of `review.rs` and
  `runner.rs`, `a_saved_correction_queues_a_review_run_that_runs_at_once` and
  `a_full_run_waits_while_its_videos_review_run_runs` in
  `apps/tbd_subtitles/src/application/tests/rendering.rs`); a failed job records its step and a
  cancelled one its kept steps (`a_failed_job_records_its_step_and_the_steps_it_kept`,
  `a_cancelled_job_keeps_its_finished_steps_and_can_be_retried` in the same file).

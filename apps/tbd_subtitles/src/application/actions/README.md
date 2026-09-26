# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── mod.rs       the module list and the re-exports `application` uses
├── queue.rs     the queue's actions, starting the next job with its options, the runner's events
├── report.rs    the selected finished job's report, read when it is selected or ends
├── review.rs    the line review: open, edit, save and queue a review run, play clips, reload
└── settings.rs  the settings page as the window opens, its actions, and its threads' answers
```

## How it works

`settings.rs` builds the settings page from the settings file (the defaults, with the reason, when
the file cannot be read) and the models its settings need. Its actions change the draft, save it
through `settings::services::page_editing`, open the desktop's chooser for a path setting, start or
stop the model download, and run the machine checks again. `poll_settings` folds the download's
progress, the checks and the work folder's size into the page; a finished download lists the
models again.

`queue.rs` applies each queue event and keeps two lanes, each with its own runner: full runs start
one after another while the queue runs, and review runs start as soon as one waits and no full
run of the same video runs. A job starts only when every model is on disk, with options built
from the saved settings (or, for a retry or a review run, the settings its `job.json` holds). The
runners' events fold into the queue: progress into the running job, the end into a finished,
cancelled or failed job, after which the step rates, the report and an open review are read again
and the next job of each lane starts.

`report.rs` reads the selected finished job's report when it is selected or a job ends.
`review.rs` opens the selected job's review at the line a finding names (or its first flagged
line), applies the owner's picks, typing and flags to the draft, and on Save or Take back writes
`review.json` and queues one review run of the video. Play starts the clip player on the open line
with 0.75 s either side, with the video's sound or the vocal stem; opening another line, Stop and
closing the review stop it.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::job_queue` (events, models,
  services); `crate::job_report::services`; `crate::line_review` (events,
  services); `pipeline` (`JobOptions`, `CancelToken`, `workers::Binaries`); `media_io::preview`
  (`Clip`); `crate::core::portal`; `crate::application` (`TbdSubtitlesApp`, `Environment`,
  `background::Chooser`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and a chooser's answer changes only the draft
  (the header of `settings.rs`); a saved correction always queues one review run, which never
  starts beside a full run of the same video (the header of `review.rs`,
  `a_saved_correction_queues_a_review_run_that_runs_at_once` in
  `apps/tbd_subtitles/src/application/tests/rendering.rs`).

# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── mod.rs       the module list and the re-exports `application` uses
├── queue.rs     the queue's actions, starting the next job with its options, the runner's events
├── report.rs    the selected finished job's report, read when it is selected or ends
└── settings.rs  the settings page as the window opens, its actions, and its threads' answers
```

## How it works

`settings.rs` builds the settings page from the settings file (the defaults, with the reason, when
the file cannot be read) and the models its settings need. Its actions change the draft, save it
through `settings::services::page_editing`, open the desktop's chooser for a path setting, start or
stop the model download, and run the machine checks again. `poll_settings` folds the download's
progress, the checks and the work folder's size into the page; a finished download lists the
models again.

`queue.rs` applies each queue event, hands the next waiting job to the runner when the queue
runs, no job runs and every model is on disk, building its options from the saved settings (or,
for a retry or a review run, the settings its `job.json` holds), and folds the runner's events
into the queue: progress into the running job, the end into a finished, cancelled or failed job,
after which the step rates are read again and the next job starts.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::job_queue` (events, models,
  services); `pipeline` (`JobOptions`, `CancelToken`, `workers::Binaries`); `crate::core::portal`;
  `crate::application` (`TbdSubtitlesApp`, `Environment`, `background::Chooser`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and a chooser's answer changes only the draft
  (the header of `settings.rs`).

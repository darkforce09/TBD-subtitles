# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── mod.rs       the module list and the re-exports `application` uses
├── queue.rs     the queue's actions: add, select, remove and undo, move, cancel, try and run again
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
start. Remove takes a row out and keeps it for Undo, which a toast offers for 6 s; a newer removal
takes the older Undo toast away, and an Undo that names another row does nothing. Start and
Resume turn the queue on, and Pause turns it off, pausing after the running full run, which a
toast names. Try Again puts a failed or cancelled job first in line and Run Again a finished one
with the settings saved now; `runner::start_now` then starts it at once when its lane is idle and
every model is on disk, without turning the queue on, and a toast says whether it started, could
not start (in red, with the reason), runs next ("from Hear the speech", while the queue runs or for
a correction run), is first in line waiting for Start Queue, or waits for the models; a video with
another run waiting or running is not put back ("… is already in the list."). Run Again compares the settings saved now
with the ones in the job's `job.json` first, and when they are the same queues nothing and says
so. Check Lines selects the job and opens its review; Show in Folder, Open in Player and a copied
subtitle path go to the desktop portal or a toast. `runner.rs` keeps two lanes, each with its own runner: full runs start one after another
while the queue runs, and review runs start as soon as one waits. Both lanes run in this one
process, so the job lock does not keep them apart: a review run waits while a full run of its
video runs, and the full lane waits while a review run of its next video runs. A pause ends once the full
lane is idle. A job starts only
when every model is on disk, with options built from the saved settings until it first starts,
and from its own `job.json` after that (a review run always), with the steps it is to run again
as `JobOptions.rerun`; starting marks the job as keeping its settings. The runners' events fold
into the queue: progress into the running job (its first event empties the steps to run again,
which the pipeline has recorded by then, so a job failing before it keeps them), the end into a
finished job, a cancelled one with the finished steps it kept, or a failed one with the step that
failed and the steps it had finished (each with its seconds, or still valid), after which the step rates, the report and an open review are read
again and the next job of each lane starts.

`report.rs` reads the selected finished job's report when it is selected or a job ends.
`review.rs` opens the selected job's review at the line a finding names (or its first flagged
line), or says in a red toast why it cannot, leaving the report as it is; closes it once its job
is no longer finished (after every queue event, so a job tried or run again shows as it is now);
applies the owner's picks, typing and flags to the draft, and on Save or Take back writes
`review.json` and queues a review run of the video, or adds the correction to the one that
waits. Play starts the clip player on the open line with 0.75 s either side, with the video's
sound or the vocal stem; opening another line, Stop and closing the review stop it.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::job_queue` (events, models,
  services); `crate::job_report::services`; `crate::line_review` (events,
  services); `pipeline` (`JobOptions`, `CancelToken`, `workers::Binaries`); `media_io::preview`
  (`Clip`); `crate::core::{portal, steps, toast}`; `crate::application` (`TbdSubtitlesApp`,
  `Action`, `Environment`, `background::Chooser`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and a chooser's answer changes only the draft
  (the header of `settings.rs`); a saved correction always queues one review run, and no run of a
  video starts beside a run of the other kind of the same video (the headers of `review.rs` and
  `runner.rs`, `a_saved_correction_queues_a_review_run_that_runs_at_once` and
  `a_full_run_waits_while_its_videos_review_run_runs` in
  `apps/tbd_subtitles/src/application/tests/rendering.rs`); a failed job records its step and a
  cancelled one its kept steps (`a_failed_job_records_its_step_and_the_steps_it_kept`,
  `a_cancelled_job_keeps_its_finished_steps_and_can_be_retried` in the same file); a job tried
  or run again starts at once without turning the queue on, and Run Again with the settings a job
  ran with queues nothing (the header of `queue.rs`,
  `run_again_with_the_settings_it_ran_with_runs_nothing` in
  `apps/tbd_subtitles/src/application/tests/rendering_queue.rs`).

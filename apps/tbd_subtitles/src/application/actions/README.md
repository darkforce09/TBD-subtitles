# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── fix_it.rs       Fix It: its view, start and Stop, the lanes it holds, and what happens when it ends
├── log_console.rs  the log window: open or close, read new lines while open, filter, clear, log file
├── mod.rs          the module list and the re-exports `application` uses
├── queue.rs        the queue's actions: add, select, remove and undo, move, cancel, try and run again
├── report.rs       the selected job's report, the finished rows' summaries, and the Overview's requests
├── review.rs       the line review: open (on a group), edit, save or keep and queue a run, clips, stills
├── runner.rs       starting the next job of each lane with its options, and the runners' events
└── settings.rs     the settings page as the window opens, edits written at once, tabs, downloads
```

## How it works

`settings.rs` builds the settings page from the settings file (the defaults, with the reason, when
the file cannot be read), the saved glossary's names and the models its settings need. An edit
and a chosen path go through `settings::services::page_editing::apply`, written at once or
refused with an error under their field; a settings file that could not be read is kept as
`settings.toml.broken` before the first write, which an info toast says; what the edit made
stale is refreshed and nothing else:
the models list when the models folder or an engine changed, a folder's size when that folder
changed, never the machine checks. `Open` shows a tab of the Settings window (the tab bar, the
banner's Details…, This Computer's link to Models); the other actions open the desktop's chooser
for a path setting, start or stop the model download, and run the machine checks again.
`poll_settings` folds the download's events (by item id), the checks and the folders' sizes into
the page. A download that ends lists the models again, runs the checks again and measures the
models folder: when everything is on disk the banner says so for a moment; a stopped one says in a
toast that it resumes where it left off, and a failed one says why in a red toast.

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
so. Check Lines selects the job and opens its review; Show in Folder and Open in Player go to the
desktop portal through `open_with_desktop`, which says so in a toast and, when the desktop answers
that it could not, in a red one; a copied subtitle path says so in a toast. `runner.rs` keeps two
lanes, each with its own runner: full runs start one after another
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
failed and the steps it had finished (each with its seconds, or still valid), after which the
step rates, the summaries of the rows of each video whose run ended, the report and an open review
are read again and the next job of each lane starts. A review run's start, and its end first,
before the next run starts, are passed to the open review of its video, whose saved lines go
from Saved to Updating and on to Updated or Failed.

`report.rs` reads the selected finished job's report when it is selected or a job ends, and with
it the job's row summary; `refresh_summaries` reads the summary of every finished row when the
window opens, and of a video's rows after each of its runs and each saved or taken-back
correction (a row whose files cannot be read has none). It applies the Overview's events: Open in
Player, Show in Folder and Open Full Report through `open_with_desktop`, the copied path's toast,
Check Lines on the lines to check, on a group (the list narrowed to it), or on every line at the
one nearest the first speech with no subtitle (Show Nearby Lines); Try Again beside a failed
language-model call, which is Try Again from `StepName::Adjudicate`: every model call and the
steps after them run again; and Fix It and its Stop.

`fix_it.rs` starts Fix It on the selected video with the Fix It model and processes saved now, on
`job_report::services::fix_it`'s thread, one run at a time and never while a run of the video
runs or its correction run waits ("Wait until this video's subtitles are updated."). While it
runs, the full lane holds a waiting run of that video, the review lane starts the first waiting
correction run whose video is neither running nor being fixed, and the queue refuses to remove,
try again or run again its row. `fix_view` gives the Overview its Fix It: hidden with nothing to
ask about, ready, off with why, or running with its stage and Stop. When a run ends
(`poll_fix`), the lines it changed are marked Saved in the open review or the parked one, one
correction run of the video carries them, and a toast says how many lines changed, kept the
owner's own correction, or whose calls failed; a stopped run says nothing changed.

`review.rs` opens the selected job's review on its lines to check, narrowed to a group when one
is named, with the edits it had when it last closed, or says in a red toast why it cannot,
leaving the report as it is; closes it once its job is no longer finished (after every queue
event, so a job tried or run again shows as it is now), parking its unsaved edits and the runs
of its saved lines under the job until it opens again; a parked review follows its video's
runs too. It applies the owner's picks, typing and flags to the open line's draft,
the list, the search and the group; on Save Correction, Looks Right (Keep Change for a Fix It
change), Undo Change or Take Back it writes `review.json` and queues a review run of the video,
or adds the correction to the one that waits
(Take Back on a line with no correction does nothing), and a line that cannot be saved (an empty text not dropped) says why in a red toast. Play starts
the clip player on the open line with 0.75 s either side, with the video's sound or the vocal
stem; Stop and closing the review stop it. Whenever the editor shows another line, by a click,
a step, a save or a filter, the clip stops and the frame at the line's start is decoded when its
video has a picture. After a run of the video ends, its lines are read again with what the owner did
carried over.

`log_console.rs` opens and closes the log window; opening reads every line the process's log
buffer still holds that the console has not, and `poll_log` reads the new ones before each frame
while the window is open, never while it is closed, so nothing logged meanwhile is lost. A level or
a search refilters the console; Clear empties the console and the log buffer (the log file keeps
every line); Open Log File opens the log file in the desktop's text editor through
`open_with_desktop`.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::job_queue` (events, models,
  services); `crate::job_report` (events, `models::finding_group`, services); `crate::line_review`
  (events, models, services); `crate::log_console` (events, models); `pipeline` (`JobOptions`, `CancelToken`, `workers::Binaries`); `media_io::preview`
  (`Clip`); `crate::core::{portal, steps, toast}`; `crate::application` (`TbdSubtitlesApp`,
  `Action`, `Environment`, `background::{Chooser, Opening}`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and an edit or a chosen path is written only
  when it can make a job's settings (the header of `settings.rs`,
  `a_bad_glossary_is_not_written_and_names_its_field` in
  `apps/tbd_subtitles/src/application/tests/rendering_settings.rs`); a saved correction always
  queues one review run, and no run of a video starts beside a run of the other kind of the same video (the headers of `review.rs` and
  `runner.rs`, `a_saved_correction_queues_a_review_run_that_runs_at_once` and
  `a_full_run_waits_while_its_videos_review_run_runs` in
  `apps/tbd_subtitles/src/application/tests/rendering.rs`); a failed job records its step and a
  cancelled one its kept steps (`a_failed_job_records_its_step_and_the_steps_it_kept`,
  `a_cancelled_job_keeps_its_finished_steps_and_can_be_retried` in the same file); a job tried
  or run again starts at once without turning the queue on, and Run Again with the settings a job
  ran with queues nothing (the header of `queue.rs`,
  `run_again_with_the_settings_it_ran_with_runs_nothing` in
  `apps/tbd_subtitles/src/application/tests/rendering_queue.rs`); Try Again from the Overview
  reruns the language-model calls, and a row finished in an earlier window shows its summary
  (`a_failed_language_model_call_needs_attention_and_try_again_reruns_the_calls`,
  `a_job_finished_in_an_earlier_window_shows_its_verdict_and_lines_to_check` in
  `apps/tbd_subtitles/src/application/tests/rendering_report.rs`); a group opens Check Lines
  narrowed to it, a save moves on while its run's status chip follows the run, and an edit waits
  while Check Lines is closed (`a_group_on_the_overview_opens_check_lines_on_that_group`,
  `use_edits_the_line_and_save_moves_on_while_the_subtitles_update`,
  `an_edit_waits_while_check_lines_is_closed` in
  `apps/tbd_subtitles/src/application/tests/rendering_review.rs`). Fix It runs one video at a time, holds
  the runs of its video, and queues one correction run of its changes
  (`fix_it_runs_on_the_video_and_queues_the_correction_run_that_times_its_changes` in
  `apps/tbd_subtitles/src/application/tests/rendering_fix_it.rs`).

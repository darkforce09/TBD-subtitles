# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── text.rs        asynchronous text review and correction runs
├── tests/         correction completion across navigation
├── automation.rs   videos from any source into the queue and the history; hand-offs, watch folders, auto-start, job-end notices, the service menu
├── fix_it/         Fix It: its view and steps, start (one, Fix All, after a job) and Stop, end, finish
├── log_console.rs  the log window: open, read lines and calls, views, filters, clear, log file
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
changed, never the machine checks; after every edit the cap on Fix It's Claude calls
(`claude_gate`) is the saved "Claude calls at once". `Open` shows a tab of the Settings window
(the tab bar, the banner's Details…, This Computer's link to Models); the other actions open the desktop's chooser
for a path setting, start or stop the model download, and run the machine checks again.
`poll_settings` folds the download's events (by item id), the checks and the folders' sizes into
the page. A download that ends lists the models again, runs the checks again and measures the
models folder: when everything is on disk the banner says so for a moment; a stopped one says in a
toast that it resumes where it left off, and a failed one says why in a red toast.

`queue.rs` applies each queue event, writes `queue.json` and lets the runner start what may
start. Remove takes a row out and keeps it for Undo, which a toast offers for 6 s; a newer removal
takes the older Undo toast away, and an Undo that names another row does nothing. Start and
Resume turn the queue on and end the owner's pause (`paused_by_owner`), and Pause turns it off,
pausing after the running full run, which a toast names, and keeps automation from starting it
again in this window. Try Again puts a failed or cancelled job first in line and Run Again a finished one
with the settings saved now; `runner::start_now` then starts it at once when its lane is idle and
every model is on disk, without turning the queue on, and a toast says whether it started, could
not start (in red, with the reason), runs next ("from Hear the speech", while the queue runs or for
a correction run), is first in line waiting for Start Queue, or waits for the models; a video with
another run waiting or running is not put back ("… is already in the list."). Run Again compares the settings saved now
with the ones in the job's `job.json` first, and when they are the same queues nothing and says
so. Check Lines selects the job and opens its review; Show in Folder and Open in Player go to the
desktop portal through `open_with_desktop`, which says so in a toast and, when the desktop answers
that it could not, in a red one; a copied subtitle path says so in a toast; Fix All goes to
`fix_it/`, which starts Fix It on every finished video with lines to fix. `runner.rs` keeps a
full lane on its own runner and four review lanes (`job_queue::services::review_lanes`), each with
a runner of its own: full runs start one after another while the queue runs, and review runs
start as soon as one waits and a review lane is idle, up to four at once and never two of the same
video. Every lane runs in this one process, which shares one handle of each job's database, so
the job store does not keep them apart: a review run
waits while another run of its video runs, and the full lane waits while a review run of its next
video runs. A pause ends once the full
lane is idle. A job starts only
when every model is on disk, with options built from the saved settings until it first starts,
and from its own `job.json` after that (a review run always), with the steps it is to run again
as `JobOptions.rerun`; starting marks the job as keeping its settings. The runners' events fold
into the queue: progress into the running job (its first event empties the steps to run again,
which the pipeline has recorded by then, so a job failing before it keeps them), the end into a
finished job, a cancelled one with the finished steps it kept, a failed one with the step that
failed and the steps it had finished (each with its seconds, or still valid), or a busy one when
another process owns the job's database (the pipeline's busy error kind, with that process's
pid); `poll_busy`, before each frame, sets a busy job waiting once `job_queue::services::busy_owner`
finds its owner gone and starts the next jobs, and a busy full run keeps the queue on. After an
end the
step rates, the summaries of the rows of each video whose run ended, the report and an open review
are read again and the next job of each lane starts. A review run's start, and its end first,
before the next run starts, are passed to the open review of its video, whose saved lines go
from Saved to Updating and on to Updated or Failed; once everything is read again, a review run's
end is passed to Fix It too (`fix_run_ended`), and so are the full runs that finished well, which
Fix It after each job starts on (`fix_after_run`).

`report.rs` reads the selected finished job's report when it is selected or a job ends, and with
it the job's row summary; `refresh_summaries` reads the summary of every finished row when the
window opens, and of a video's rows after each of its runs and each saved or taken-back
correction (a row whose files cannot be read has none). It applies the Overview's events: Open in
Player, Show in Folder and Open Full Report through `open_with_desktop`, the copied path's toast,
Check Lines on the lines to check, on a group (the list narrowed to it), or on every line at the
one nearest the first speech with no subtitle (Show Nearby Lines); Try Again beside a failed
language-model call, which is Try Again from `StepName::Adjudicate`: every model call and the
steps after them run again; and Fix It and its Stop, on the selected job (the Overview's own
come as `Action::FixIt` and `Action::StopFix`, naming the job it shows).

`fix_it/` starts Fix It on a video (its Overview's button, `Action::FixIt` naming the job it
shows), on every finished video with lines to fix (the sidebar's Fix All), or, with Fix It after
each job on, on a full run that just finished well (`fix_after_run`, from the runner once the
summaries are read again), with the Fix It model and processes saved now, on
`job_report::services::fix_it`'s threads. Many runs go at once, one per video, each on its own
thread; a video already being fixed says "Fix It is already fixing this video.", and none starts
while a run of the video runs or its correction run waits ("Wait until this video's subtitles are
updated."). Every run's `claude` calls take a slot at the one gate the window holds
(`claude_gate`), whose cap is the saved "Claude calls at once" (`fix_calls`) and changes at once
when it is edited; the calls wait in the order their runs started, and `processes` caps each run
alone. Fix All starts on the rows of Done that finished, have findings Fix It would ask about
(`RowSummary::fixable`) and nothing against a run, one per video, oldest finished first, then says
"Fix It started on 3 videos." (or, in red, why the first could not start). While a run goes, the
full lane holds a waiting run of its video, the review lanes start the first waiting correction
runs whose videos are neither running nor being fixed, and the queue refuses to remove, try again
or run again its row. `fix_view` gives the Overview its Fix It: hidden with nothing to ask about,
ready, off with why, running with its stage, Stop and whether every call of it waits for a free
slot, or updating while its correction run waits or runs; `fix_steps` gives the sidebar each video
Fix It fixes with its step of four; the window redraws every second while a run goes, so the
waiting note stays current. When a run ends (`poll_fix`, every run each frame) with changes, the
lines it changed are marked Saved in the open review or the parked one, one correction run of the
video carries them, and the run waits in `fix_followups`, saying nothing yet; when that correction
run ends (`fix_run_ended`, from the runner once the summaries and the report are read again), or
at once when nothing changed, the run finishes (`fix_finished`): a green toast for 8 s says what
Claude did ("Dressrosa 12 is fixed: Claude changed 17 lines. The subtitles are ready.", "…; 1
problem is left for you.", or "Claude checked Dressrosa 12: every line was already right."), how
many lines kept the owner's own correction and how many calls failed, with See Changes
(`Action::SeeFixChanges`: the job selected and Check Lines opened on Changed by Claude) when lines
changed; the job is marked just fixed (`just_fixed`, until another job is selected) for its result
card; and while the window is unfocused or minimized (`presence`), the desktop gets a notification
("Dressrosa 12 is fixed", "Claude changed 17 lines. The subtitles are ready.") and the window asks
for its attention. A correction run that fails or is stopped forgets the run; a stopped Fix It
says nothing changed on that video ("Fix It stopped on Dressrosa 12. …"), a failed one says
why ("Fix It failed on Dressrosa 12: …"), and one refused because another process runs the video
says so, in an information toast and not as a failure ("Dressrosa 12 is busy: process 4242 runs
it; Fix It again once it ends.").

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

`automation.rs` queues the videos of every `QueueVideos` (dropped, chosen, named on the command
line, handed over, found in a watch folder) through `queue_editing::add_videos` and records each
video queued in the queued history (`queued_videos.json`, seeded from the kept queue when the
window opens, written whenever it grows). `poll_automation` runs before each frame, and while the
window is minimized: it keeps the folder watcher on the saved watch folders, takes each hand-off
of a later start (its videos queued with a toast, the queue started when it asks, the window
brought forward when it asks), queues the watch folders' videos that the history does not hold,
that have no subtitle file and that do not wait or run (with a toast), and starts the queue that
waited for the models once they are on disk. A start from automation goes through
`JobQueueEvent::Start`, as Start Queue does, unless the owner pressed Pause in this window; while
a model is missing and the window is away it tells the desktop "1 video queued" with "Download
the missing models in Settings to start them.". `runner.rs` hands it the notice of each full run
that finished or failed (`job_queue::services::job_notice`), which it sends to the desktop, with
the window's attention asked for, only while the window is away. `install_right_click` writes
Dolphin's service menu as the window opens and turns how that went into the Automation tab's
`RightClickEntry`, logged.

`log_console.rs` opens and closes the log window; opening reads every line and model call the
process's log buffer still holds that the console has not, and `poll_log` reads the new ones
before each frame while the window is open, never while it is closed, so nothing logged meanwhile
is lost. It switches the view, refilters by level, writer or search, opens a line in the detail
panel or a call on the right, and Show Model Call opens the Model Calls view on a line's call.
Clear empties the view shown, in the console and in the log buffer (the log file keeps every
line); Open Log File opens the log file in the desktop's text editor through `open_with_desktop`.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::job_queue` (events, models,
  services); `crate::job_report` (events, `models::finding_group`, services); `crate::line_review`
  (events, models, services); `crate::log_console` (events, models); `pipeline` (`JobOptions`, `CancelToken`, `workers::Binaries`); `media_io::preview`
  (`Clip`); `crate::core::{format, portal, service_menu, single_instance, steps, toast}`;
  `crate::application` (`TbdSubtitlesApp`, `Action`, `Environment`, `HandOffs`,
  `background::{Chooser, Opening}`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and an edit or a chosen path is written only
  when it can make a job's settings (the header of `settings.rs`,
  `a_bad_glossary_is_not_written_and_names_its_field` in
  `apps/tbd_subtitles/src/application/tests/rendering_settings.rs`); a saved correction always
  queues one review run, and no run of a video starts beside another run of the same video, while
  up to four correction runs of different videos run at once (the headers of `review.rs` and
  `runner.rs`, `a_saved_correction_queues_a_review_run_that_runs_at_once`,
  `a_full_run_waits_while_its_videos_review_run_runs` and
  `four_correction_runs_run_at_once_but_never_two_of_one_video` in
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
  `apps/tbd_subtitles/src/application/tests/rendering_review.rs`). Fix It runs on many videos at
  once, one run per video, under one cap on Claude calls, holds the runs of its video, and queues
  one correction run of its changes; it finishes once that run ended, or at once with nothing
  changed, telling the desktop only while the window is away, and a failed correction run
  finishes nothing (`fix_it_runs_on_the_video_and_queues_the_correction_run_that_times_its_changes`,
  `fix_it_finishes_once_its_changes_are_in_the_subtitles`,
  `fix_it_with_nothing_to_change_finishes_at_once_and_leaves_the_desktop_alone_in_front`,
  `a_failed_correction_run_finishes_nothing` in
  `apps/tbd_subtitles/src/application/tests/rendering_fix_it.rs`); Stop ends only its own
  video's run, Fix All and Fix It after each job start on exactly the finished videos with lines
  to fix, and the cap follows its setting at once (`two_videos_fix_at_once`,
  `stop_ends_only_its_own_video`,
  `fix_all_starts_every_finished_video_with_lines_to_fix_and_hides_once_all_are_fixing`,
  `fix_after_each_job_starts_when_a_full_run_finishes`, `fix_after_each_job_off_starts_nothing`,
  `claude_calls_at_once_applies_at_once` in
  `apps/tbd_subtitles/src/application/tests/rendering_fix_many.rs`). Automation never starts a
  queue the owner paused until Start, a watch folder never queues a video queued before, with
  subtitles or in line, and the desktop hears of a job's end only while the window is away (the
  header of `automation.rs`,
  `automation_never_starts_a_queue_the_owner_paused_until_start`,
  `a_watch_folder_queues_only_videos_never_queued_without_subtitles`,
  `a_job_that_ends_while_the_window_is_away_tells_the_desktop`,
  `a_job_that_ends_with_the_window_in_front_leaves_the_desktop_alone` in
  `apps/tbd_subtitles/src/application/tests/rendering_automation.rs`).

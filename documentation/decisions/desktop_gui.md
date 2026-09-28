**Status:** live

# Decisions: desktop GUI

The decisions the desktop window led to: how it plays clips and hands videos to other programs,
where models live and how they arrive, when a job passes the quality check, how the owner's
corrections are timed, how the window looks, which settings a job runs with, where Settings open,
how correction runs show, how a job is tried again, how a failed language-model call is retried,
how a change of the settings applies, what Fix It does, and what the log window shows, model
calls too. The [decision log](/documentation/decisions/) says how entries are written.

### 2026-09-26 — Clips play through FFmpeg, not libmpv

**Context:** Reviewing a flagged line means hearing it, and seeing who speaks. The GUI feature
named libmpv for the picture, which the app would link; the app may run only FFmpeg, ffprobe and
`claude`, and no audio crate is in the tree (cpal would link ALSA).

**Decision:** FFmpeg plays a clip itself: one FFmpeg sends the clip's sound to PipeWire through its
`pulse` output device, and a second pipes small raw RGBA frames (about 480 lines high) that the
window draws, paced to the start of the sound. No new program runs and no native library links.

**Consequences:** The picture is small and a clip cannot be sought inside; it is played again from
its start. The machine check reports an FFmpeg without the `pulse` device, where clips play
without sound.

**Supersedes:** none.

### 2026-09-26 — The desktop portal chooses files and opens videos

**Context:** The queue needs a file and folder chooser, and the report a button that opens the
video in VLC. Starting VLC would be a fourth external program; `rfd` loads `libdbus` at run time.

**Decision:** The app asks the XDG desktop portal over D-Bus, through `ashpd` (zbus, pure Rust,
async-io): its FileChooser for videos, folders and glossary files, and its OpenURI to open a video
in the desktop's default player, VLC on the owner's PC, or a folder in the file manager. The app
starts no program for either.

**Consequences:** The dialogs are the desktop's own. The portal cannot start a video at a given
time, so the report names the flagged times and the review plays its own clips.

**Supersedes:** none.

### 2026-09-26 — The 120-minute test joins three episodes

**Context:** The pipeline's acceptance asks for a 120-minute test file; no video in the media
folder is longer than 43.6 minutes.

**Decision:** Dressrosa 39, 48 and 40 are joined by stream copy with FFmpeg's concat demuxer into
one 128.9-minute file under `~/.local/share/tbd-subtitles/test-media/`. The episodes are only read
and the media folder is not touched.

**Consequences:** The test runs the same encoding as the batch. Its result is the
[120-minute test](/documentation/research/long_video_120min.md).

**Supersedes:** none.

### 2026-09-26 — The owner's corrections are timed by a review step

**Context:** A corrected line must be timed again and the subtitle file rewritten. Running
alignment again over the whole job needs the GPU, which a batch job holds, and may move the lines
around the corrected one.

**Decision:** An eighteenth step, `review`, runs after alignment. It reads the owner's corrections
from `review.json` and aligns each corrected utterance alone with Parakeet-CTC on the CPU, in a
worker of the main binary; every other line keeps its times. Cues, the quality check and the output
then run again. Corrections survive any later run of alignment.

**Consequences:** A correction takes seconds and runs beside a batch job. Text the owner types is
the one exception to "every subtitle word comes from what a speech engine heard": the owner heard
it.

**Supersedes:** none.

### 2026-09-26 — A short interjection shares a neighbour's cue

**Context:** The 120-minute test left three one-word cues under 20 frames ("ha!", "Ah!", "Huh?"),
each between two cues too full to share one with; the layout rule forbids them, so such an
episode fails the quality check.

**Decision:** When a cue stays too short after the sharing rule, its text is merged into the cue
before or after it, the joined text broken again into at most two lines of 42 characters within
the reading speed; when no merge fits, the short cue grows into the gaps around it down to the
2-frame gap.

**Consequences:** The cue stage's revision rises, so every job redoes its cues, quality check and
output once.

**Supersedes:** none.

### 2026-09-26 — Models live in the settings' models folder and download from the Settings page

**Context:** The roadmap left open where models live and how their first download is shown. Only
the stack spike tool downloaded them, and the tasks named their model folders in three places.

**Decision:** The models folder is a setting (`models_dir`, default
`~/.local/share/tbd-subtitles/models/`); a job records the folder it read in `job.json`, and no
fingerprint covers it. `pipeline::models::required` is the one list of the folders a job's settings
need. The window's Settings page lists them and the runtime archives the workers load (the CUDA 13
libraries and ONNX Runtime; not the build-only toolkit), each with its size and whether it is on
disk, and downloads the missing ones with progress and a stop button; a job does not start while
one is missing. The runtime folder stays `~/.local/share/tbd-subtitles/runtime/`.

**Consequences:** Every download is pinned by size and SHA-256 and resumes a stopped part file.
Changing the models folder changes no job's output.

**Supersedes:** none.

### 2026-09-26 — A job passes the quality check on five rules

**Context:** The batch's acceptance asks that each report pass the quality check; the report listed
findings but said nothing of passing.

**Decision:** A job passes with no layout violation (overlap, under 20 frames, a line over 42
characters, a third line, an empty cue, a cue past the end), no heard speech left without a cue, no
failed language-model call, no aligner offset of 30 ms or more, and at least 95 % of cues at or
under 20 characters per second. The other findings (fast cues, unsure lines, novel words, dropped
agreed words, weak timing, gaps under 2 frames) are for review and never fail a job.

**Consequences:** The command line and the window say whether a job passes and name each failed
rule. Each finding about one line names its utterance, so the window can open the line.

**Supersedes:** none.

### 2026-09-26 — One GPU worker at a time on the machine

**Context:** The window's queue and a command-line run could each start a GPU worker, and two
models together overrun the card's 5.5 GB.

**Decision:** Every GPU worker first takes an exclusive `flock` on `gpu.lock` in the app data folder
and holds it while it runs; a run that finds it held waits, says so, and can be cancelled while it
waits. The kernel frees the lock when its holder dies.

**Consequences:** A second run of the app never loads the GPU beside the first; it waits instead.

**Supersedes:** none.

### 2026-09-26 — The owner chooses SRT, WebVTT or ASS

**Context:** The settings ask for an output format; only an SRT writer existed. VLC loads one
subtitle file per video.

**Decision:** The output format is a setting (`output_format`, SRT by default) that only the output
step reads. When it changes, the job's file of the old format moves to the job's `backup/` folder,
so one subtitle file stays beside the video.

**Consequences:** Changing the format reruns only the output step. The ASS file carries one
dialogue style; positioned sign styles come with the Japanese on-screen text feature.

**Supersedes:** none.

### 2026-09-28 — The window follows the desktop's colour scheme in Adwaita Sans under X11

**Context:** The owner approved a macOS-like mockup of the window, in light and dark, with
icons. egui draws in its own dark theme and fonts and knows nothing of KDE's colour scheme. On
Wayland, winit 0.30 delivers no dropped files (only its X11 backend emits them), places no window,
and with vsync on a minimised second window stops the main window's frames; XWayland at the
owner's scale factor of 1 draws the same pixels, as smoothly. Each phase of the redesign needs
screenshots of the real window to compare with the mockup.

**Decision:** The window follows the desktop's colour scheme through the desktop portal's
`settings` interface (the `ashpd` feature `settings`, its change stream read with
`futures-util`): it reads the scheme before the first frame, switches on every change, and treats
"no preference" as light. The palettes are the mockup's tokens; KDE's accent colour is not
followed. The text is Adwaita Sans read from `/usr/share/fonts` at start, with semibold and bold
made from its weight axis, and Adwaita Mono for monospace text; egui's fonts stand in when a file
is missing. Icons come from
`egui-phosphor` 0.14 (regular weight; the font ships inside the crate, MIT or Apache-2.0), so no
binary file is committed. The window runs under X11: `launch` sets winit's `with_x11` through a
direct `winit` dependency pinned to eframe's version. The dev dependencies `egui_kittest` 0.36
(features `wgpu` and `eframe`) and `image` 0.25 (`png`) serve an ignored test that renders the
real window offscreen to PNG files for review.

**Consequences:** Files dropped from Dolphin reach the queue, and a second window can be placed
over the main one; the owner confirms a real drop. The window shows X11's generic icon until it
sets one of its own. At a fractional scale factor XWayland may draw text softly; the owner's
screens are at scale 1. Test builds compile wgpu for the snapshot test.

**Supersedes:** none.

### 2026-09-28 — A job keeps its own settings once it has started

**Context:** A job reads its settings when it starts and records them in its `job.json`; each
step's output is valid only for the settings it was made with. The window remembered whether a job
had run only until it closed, so a failed or cancelled job retried in a later window took the
settings saved since, and its finished steps ran again under them. A retry is meant to resume, and
the window is to offer running a job again with the settings saved now as a command of its own.

**Decision:** A job takes the settings saved now until it first starts. From then on every run of
it, a retry after a failure or a cancel included, takes the settings in its own `job.json`, as a
review run always does. `QueueItem.keep_settings` records this, and `queue.json` keeps it across
windows; in an older file without the field, a job that no longer waits has started and keeps its
settings. Running a job again with the settings saved now is a separate, explicit command that
names the steps to run again: they reach the pipeline as `JobOptions.rerun`, as the CLI's
`--rerun` does, and leave the queue once the pipeline has recorded them in `job.json`, so a run
that fails before that keeps them.

**Consequences:** A retry resumes after the finished steps the job kept, whatever changed in the
settings since; a settings change reaches new jobs and the runs the owner asks for again. A job
that failed before writing its `job.json` has no settings of its own and takes the saved ones.
`queue.json` also keeps each failed job's step, message and kept steps, a cancelled job's kept
steps, and a review run's count of corrections.

**Supersedes:** none.

### 2026-09-28 — Settings open in a window of their own

**Context:** The approved mockup opens Settings from a gear in the toolbar, in a window of their
own that KDE decorates, while the main window keeps the list and the selected job. The window had
two pages, Jobs and Settings, with tabs, so opening Settings hid the queue. eframe draws a second
native window with `show_viewport_immediate`; spike S3 showed that on Wayland with vsync on a
minimised second window stops the main window's frames, which does not happen under X11, and that
only X11 lets the app place the window.

**Decision:** Settings open in a second native window (`show_viewport_immediate`, 660 by 600,
at least 480 by 360), from the gear or Ctrl+,, centred over the main window when they open. The
window stays where the owner puts it until it is closed with its title bar's ✕; asking for it
while it is open brings it to the front. The pages and their tabs are gone: the main window always
shows the list and the selected job. Until the settings are redone, the window shows the settings
page as it was, with Save and Revert. Where the backend draws one window only, as in the headless
tests, egui shows it as a window inside the main one.

**Consequences:** The queue and the running job stay in view while the owner changes a setting.
The window runs under X11, as decided for the look, so it can be placed; on Wayland KWin would
place it. The rendering tests see the settings inside the main window, cut at its height.

**Supersedes:** none.

### 2026-09-28 — Correction runs show in their video's row

**Context:** Saving a correction queues a review run (a correction run) of the video, which
re-times the corrected lines and rewrites the subtitle file. The queue showed each as a row of its
own ("Dressrosa 13.mp4 · 2 corrections"), so a video with corrections had two or more rows, and
the correction runs counted as waiting jobs. The approved mockup shows one row per video, which
reads "Updating subtitles · 2 corrections" while its correction runs.

**Decision:** The sidebar builds one row per video. A correction run that waits, runs or has
finished folds into the row of its video's newest finished full run (else its last full run),
which reads "Updating subtitles · N corrections", N being the corrections the waiting and running
runs carry, with a spinner for its mark, and whose menu offers Stop Updating Subtitles while one
runs; a row leaves the list with the correction runs folded into it, and not while one of them
runs. Ended jobs stand newest first in the queue, so the Done section lists them that way. A correction run that failed or was cancelled keeps a row of its
own ("Dressrosa 13 · 1 correction"), so the owner sees it and can try it again; so does one whose
video has no full run in the list. The toolbar's queue button counts full runs only: a waiting
correction run does not enable Start Queue, since correction runs start by themselves.

**Consequences:** The list holds one row per video, as the owner thinks of it; the finished
correction runs stay in the queue, folded out of sight, and leave with their row. The queue file
is unchanged.

**Supersedes:** none.

### 2026-09-28 — Try Again resumes after the kept steps and starts at once when nothing runs

**Context:** A failed or cancelled job keeps its finished steps, and a retry resumes after them
with the job's own settings. Retry put the job back to waiting where it stood, so it ran only when
the owner started the queue and the jobs before it had run. The approved mockup's Try Again puts
the job first in line and starts it at once when nothing runs, and its Run Again runs a finished
job again. The owner chose the label Run Again with Current Settings: only the steps the changed
settings touch run, and a toast says so when nothing changed.

**Decision:** Try Again puts a failed or cancelled job back to waiting, first in line among the
runs of its kind, keeping its own settings and the steps it kept, with a step to run again when
one is named. Run Again with Current Settings does the same for a finished job with the settings
saved now, so the pipeline runs only the steps whose settings changed; when the settings saved now
are the ones in the job's `job.json`, it queues nothing and a toast says so. Either starts the job
at once when its lane is idle and every model is on disk, without turning the queue on, so the
queue does not go on to the next video; otherwise it waits first in line. A toast says which
happened ("Trying Dressrosa 19 again from Hear the speech."; "Dressrosa 19 runs next, from Hear
the speech." while the queue runs; "Dressrosa 19 is first in line. Press Start Queue to run it.";
"Dressrosa 19 waits for the models."), in red with the reason when the job could not start.
Neither puts a job back, and Undo does not restore a row, while another run of the same kind of
its video waits or runs: a toast says the video is already in the list. Pause After This Video turns the queue off while
the running full run finishes, and Resume Queue turns it on again.

**Consequences:** A job that failed can be tried again at once without starting the whole queue.
The toolbar tells Pause After This Video and Resume Queue apart through the queue's pausing
state, which lasts until the full lane is idle and is not kept across windows.

**Supersedes:** none.

### 2026-09-28 — A failed language-model call is retried by running adjudication again

**Context:** The quality check fails a job when a language-model call failed (`failed_call`): the
lines of that batch may be missing their settled words. The Overview names the problem ("1
language-model call failed") and the approved mockup puts Try Again beside it. The job has
finished, so there is no failed step to resume from, and the pipeline knows no single batch to
call again: adjudication, the second look at unsure lines and the sound cues each make their own
calls, and every step after them builds on what the calls returned.

**Decision:** Try Again beside a failed language-model call is Try Again from adjudication: the
finished job goes back to waiting, first in line and keeping its own settings, with
`StepName::Adjudicate` as its step to run again, and starts at once when its lane is idle. The
pipeline runs adjudication again and, as always, every step that reads a step run again: the
second listen to the unsure lines, the second look and the sound cues (so every model call runs
again), then the timing, the review step, the cues, the check and the subtitle file. The steps
before adjudication (the decoding, the voices, both speech engines, the sounds) stay valid and are
not run.

**Consequences:** A retry costs the model calls of the whole job, not only the failed batch, and
the second listen to the unsure lines on the GPU; it needs no new pipeline option. The owner's corrections in `review.json`
are applied again by the review step, as after any run.

**Supersedes:** none.

### 2026-09-28 — Settings apply as they change

**Context:** The Settings window showed one page with Save and Revert: the owner edited a draft,
and nothing reached `settings.toml` until Save, which refused a draft whose glossary could not be
read. The approved mockup splits the settings into four tabs (General, Engines, Models, This
Computer) with no Save: its footer says "Changes save as you make them. They apply to videos that
haven't started.", and a bad glossary file shows a red error under the glossary and is not used.
A job takes the settings saved when it first starts and keeps them after that, so a saved change
never reaches a job that has started.

**Decision:** Each change is written to `settings.toml` at once, as the saved settings with that
one field changed; it is refused, with nothing written and a red error under the field until an
edit of it is written, when a number is out of its range or not finite (1–16 processes, a cut
score of 1–100), when the models folder changes while a download runs ("Stop the download
first."; its buttons are off meanwhile), or when a new glossary cannot be read ("Can't use
names.json: … The glossary was not changed."). The glossary is read only when it changes, so an
unreadable one blocks no other edit and its error stays under it. A list, the subtitle format and
a stepper's arrows send their change on the click; a text field and a stepper's typed number on
Enter, when the field loses the focus, or when the window closes with it typed. Before the first
write over a settings file that could not be read, the file is kept beside itself as
`settings.toml.broken`, which an info toast says. An edit refreshes only what it made stale: the
models list when the models folder or an engine changed, a folder's size when that folder
changed, never the machine checks. Save, Revert and the draft are gone. A banner under the
toolbar says what is missing, counting models and runtime libraries apart, with Details… (the
Models tab) and Download; it shows the download's progress with Stop, and "All models are on
disk." for 4 s after a download that brought everything onto disk. Download events name their
item by id.

**Consequences:** No change is lost by closing the window, and nothing out of range is written.
A settings file that could not be read is never lost: its first edit keeps it as
`settings.toml.broken` and writes the defaults with the owner's change. A glossary that can no
longer be read still stops a job from starting, with its error under the glossary. The models
list can be planned again while a download runs (another engine chosen) and its items are still
marked right. The tests hold it: `a_valid_edit_is_written_at_once`,
`a_bad_glossary_is_not_written_and_names_its_field`,
`numbers_out_of_range_or_not_finite_are_never_written` and
`an_unreadable_settings_file_is_kept_before_the_first_write` in
`apps/tbd_subtitles/src/settings/services/tests/page_editing.rs`,
`a_list_planned_again_during_a_download_is_marked_by_id` in
`apps/tbd_subtitles/src/settings/services/tests/model_downloads.rs`, and the rendering tests in
`apps/tbd_subtitles/src/application/tests/rendering_settings.rs`.

**Supersedes:** the Save and Revert of 2026-09-28 — Settings open in a window of their own.

### 2026-09-28 — Fix It: a stronger model fixes the flagged lines in three passes

**Context:** A finished job lists its problems and the lines worth a listen, and the owner fixed
each by hand in Check Lines. The owner asked for a button that fixes them automatically with a
stronger model than the run's (Opus beside the run's Sonnet), chosen in Settings, Engines. The
owner was wary of blind cutting (is a repeat a stray, or part of the sentence?) and asked for
passes: the whole video read for context first, then the fixes, then a check of each fixed line
against the unfixed one in the show's context; and for that context to come without the owner
typing anything, and without the model overthinking. Law 8 holds: every subtitle word comes from
what a speech engine heard.

**Decision:** Fix It asks about every finding but the aligner's offset and failed calls, in three
passes of `claude -p` with no tools. The first reads the video's file and folder names, the
glossary and every line, and uses the model's own knowledge of the series for a brief (show,
episode, cast, scenes, speech habits that are meant, and lines out of place); no setting holds
context and nothing is looked up online. The second fixes one family at a time (words, timing and
layout, reading speed), each answer held by a guard: no word no engine heard in the line or next
to it and not in the glossary, no empty line unless dropped, no sheet notation, a reason, and for
reading speed words taken out only. The third, a judge, accepts a change only when it reads right
against the line before it; nothing else is kept. A kept change becomes a correction marked
`fix_it` with the model and the reason, and the existing correction run times it. The owner's
corrections always win: Fix It never asks about a line the owner settled, and the window and Fix
It change `review.json` only under its lock, each reading it from disk. Each Fix It change waits
in a Changed by Claude group of Check Lines: Keep Change makes it the owner's (`kept_fix_it`),
Undo Change saves the language model's reading as the owner's. The quality check checks the words
of a change the owner has not kept again, and counts such lines apart. Fix It runs on a thread of
the window, one video at a time, never beside a run of its video; it keeps each answered call in
`fix/calls/`, so Stop, or a closed window, loses nothing that was paid for. `tbd-subtitles fix`
runs it without a window. The Fix It model is `fix_model` in `settings.toml`, `opus` by default.

**Consequences:** Removing heard words is no longer the owner's alone: Fix It may take out
filler, stutters, empty repeats and strays, but only through the judge, and every such line is
listed for the owner with what the app had. A batch of Fix Its cannot be queued; it is one button
per video. The quality check reads the re-decodes now, so its revision went to 4 and existing jobs
run their check and output again on their next run, in seconds. On Dressrosa 12 the aligner could not time
three changed lines alone and the backbone's times moved one subtitle 0.6 s late, so the review
step now carries a corrected line's earlier aligned times over to the words that stayed or
replaced others, for the owner's corrections too, and keeps the backbone's times for lines the
aligner never timed. A run of Opus uses the owner's
plan beside any Sonnet run of another video. The tests hold it:
`a_word_no_engine_heard_is_refused_and_the_line_stays` and
`only_the_judge_s_accepted_changes_are_kept` in `crates/stages/src/fix_it/tests/fix_it.rs`,
`a_correction_the_owner_saves_during_the_run_wins` in `crates/pipeline/src/fix_it/tests/fix_it.rs`,
`owner_lines_are_settled_and_fix_it_lines_are_checked_again` in
`crates/pipeline/src/tasks/tests/layout.rs`, and
`apps/tbd_subtitles/src/application/tests/rendering_fix_it.rs`.

**Supersedes:** none; it adds a writer of `review.json` beside the owner's line review.

### 2026-09-28 — A log window shows everything the app does

**Context:** The owner asked for a button that opens a log console in a window of its own and
shows everything that is happening. The window logged about ten events, all of them its own
errors, to stderr and to `~/.local/state/tbd-subtitles/tbd-subtitles.log`; the library crates
logged nothing. A job's events reached the window but only its current line showed, and a child
process's stderr (workers, FFmpeg, `claude`) was kept until it exited and shown only on failure.

**Decision:** One `tracing` subscriber feeds three outputs in the window's run: stderr at `info`,
the log file and an in-memory buffer of the newest 20,000 lines (`core::log_buffer`) at debug for
this workspace's crates and info for the rest, all three filtered by `RUST_LOG` when it is set.
Every source becomes a `tracing` event rather than a channel of its own: `child_process` logs each
child's start with its command line (long or multi-line arguments, such as a prompt, as their
size), each stderr line as it arrives, and its end; a worker writes its own debug lines to stderr,
which arrive that way; the job runner logs each job event (a step's advance once per tenth); Fix
It logs its passes; the pipeline logs where each step runs, the CUDA runtime, the GPU lock and
the report; the `claude` backend logs one summary line per call (model, input lines, seconds,
tokens, cost), never the prompt or the answer, as the owner chose; and the window logs each action
it applies, cut to 160 characters. The log window is a second native window like Settings, opened
from a terminal button left of the gear or Ctrl+L, with the levels (Errors and Warnings counted,
Info, Debug), a search, Copy, Clear and Open Log File; it follows the newest line until the owner
scrolls up. While it is open it reads the new lines four times a second.

**Consequences:** The log file now holds the debug lines too, so it grows faster, and each launch
still empties it. FFmpeg's and the workers' stderr cost a `tracing` event per line; with no
subscriber (the repository tools) they cost nothing. The `process` and `fix` commands print what
they did before, since their stderr stays at `info`. The tests hold it:
`a_captured_child_logs_its_start_its_stderr_lines_and_its_exit` in
`crates/child_process/src/tests/trace.rs`, `a_step_advance_is_logged_once_per_tenth` in
`apps/tbd_subtitles/src/job_queue/services/tests/progress_log.rs`, and
`apps/tbd_subtitles/src/application/tests/rendering_console.rs`.

**Supersedes:** none.

### 2026-09-28 — Model calls show in the log window, never in the log file

**Context:** The owner asked to see exactly what is sent to the language model and what comes
back, in the log window but apart from the other lines, so thousands of prompt lines never flood
them; and for the window to make clear whether the AI, the app, a job or a program did something,
grouped so it is clear what is happening and why, with no line wider than the window. The owner
chose one row per line with the whole line a click away, a header per step with a chip for who
wrote each line, and calls kept in the window only. Adjudicate, Readjudicate and SoundCues call
`claude` in worker processes, whose logs the app saw only as stderr text; Fix It calls it in the
app.

**Decision:** `inference::llm::call_log` numbers every call and, after it, emits its summary line
(`info`, with the call's number and why it was made) and its whole exchange: the system prompt,
the message, the schema, the answer or what `claude` printed, tokens, cost and time
(`job_model::model_call::ModelExchange`, as a `trace` event with target `model_exchange`, built
only when a subscriber wants it). Each stage says why it calls (`llm::purpose`): the batch, the
round of words heard again, the sound window, Fix It's pass and call. A worker writes each
exchange to its stdout as one `model-call <json>` line, which the job runner reads beside the
`progress` lines (`Progress::ModelCall`) and logs again in the app. The app's stderr and log file
drop the `model_exchange` target; the window keeps the newest 500 calls in memory, in a Model
Calls view beside the Activity view. For grouping, the job runner, the pipeline and Fix It open
`job{video}` and `step{step}` spans, `child_process` logs a child's lines in the span it was
started in, and the log window's layer gives each line its video and step; a worker logs with
its targets, and the window reads its lines back. Each line shows a chip for its writer, found
from its target (AI, Job, Program, App), and a header row names the video and step in words each
time the work moves on. A line is one row, cut with `…`; a click shows it whole in a panel below.

**Consequences:** A prompt or an answer is never written to disk by the log, and a call made
while the window is closed is still there when it opens, up to 500. The log file's lines carry
their spans (`job{video=…}:step{step=…}:`), so they say where they belong too. The worker's
stdout carries a second kind of line, which the `process` command ignores. The tests hold it:
`a_model_call_is_kept_as_a_call_under_its_step_and_never_as_a_line` in
`apps/tbd_subtitles/src/core/log_buffer/tests/layer.rs`,
`a_model_call_line_becomes_a_model_call_and_a_broken_one_a_short_message` in
`crates/pipeline/src/workers/tests/workers.rs`,
`a_child_logs_its_stderr_and_its_end_in_the_span_it_was_started_in` in
`crates/child_process/src/tests/trace.rs`,
`text_outputs_never_carry_a_model_call_and_the_window_always_does` in
`apps/tbd_subtitles/src/core/tests/logging.rs`, and
`apps/tbd_subtitles/src/application/tests/rendering_console.rs`.

**Supersedes:** none; it adds to "A log window shows everything the app does".

**Status:** live

# Decisions: desktop GUI

The decisions the desktop window led to: how it plays clips and hands videos to other programs,
where models live and how they arrive, when a job passes the quality check, how the owner's
corrections are timed, how the window looks, which settings a job runs with, where Settings open,
how correction runs show, how a job is tried again, and how a failed language-model call is. The
[decision log](/documentation/decisions/) says how entries are written.

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

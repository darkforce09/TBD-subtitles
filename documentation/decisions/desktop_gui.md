**Status:** live

# Decisions: desktop GUI

The decisions the desktop window led to: how it plays clips and hands videos to other programs,
where models live and how they arrive, when a job passes the quality check, how the owner's
corrections are timed, and how the window looks. The [decision log](/documentation/decisions/) says how entries are
written.

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

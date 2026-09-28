**Status:** live

# Desktop GUI

The window the owner uses to queue videos, watch progress, read job reports and fix the lines the
pipeline was unsure about; milestone M2. What exists now: the window opens (`tbd-subtitles` or
`tbd-subtitles gui [VIDEO]...`) with a toolbar across the top, a sidebar of videos on the left
and the selected job on the right. The sidebar lists the videos given on the command line, dropped
onto the window or added with Add Videos… and Add Folder… through the desktop's chooser (files,
or a folder for its videos without subtitles), one row per video in the sections Now, Up Next and
Done; a row leaves the list by its ✕, its menu or Delete, and Undo puts it back. Settings open in
a window of their own from the gear (Ctrl+,): they edit `settings.toml`, list and download the
models, and check the machine. Start Queue runs the waiting jobs one at a time, with the stage at
work, each step's progress under its plain title and the time left; Pause After This Video stops
the queue once the running video ends. A running job can be cancelled and an ended one tried
again, a failed job names the stage and step it failed at, a failed or cancelled one says how many
finished steps it kept, and the queue is kept across windows. A finished job shows its report:
whether it passes the quality check, its findings with their times, its steps, and buttons that
open the video, its folder and `report.md` through the desktop. From the report the owner opens the line review: the flagged
lines (or every line), each with its clip (the video's sound or the voices alone, and a small
picture), what every engine heard, the text and its flags. The owner picks a reading or types the
line, and Save and time again writes `review.json` and starts a review run at once, which
re-times the corrected lines and rewrites the subtitle file; Take the correction back undoes one.
The video's row shows the review run ("Updating subtitles · 1 correction") until it ends.

## Where it lives

- Code: `apps/tbd_subtitles/`, subcommand `gui`, built with eframe (egui) on the glow renderer:
  the shell in `apps/tbd_subtitles/src/application/`, and one feature folder each for the queue
  (`apps/tbd_subtitles/src/job_queue/`), the report (`apps/tbd_subtitles/src/job_report/`),
  line review (`apps/tbd_subtitles/src/line_review/`) and settings
  (`apps/tbd_subtitles/src/settings/`).
- Entry: `tbd-subtitles gui` from a terminal on the host; a desktop entry "TBD Subtitles" is
  planned with the automation feature.
- Related: [automation](/documentation/features/automation.md) feeds the same job queue.

## Behaviour

1. **Queue.** Add videos by file picker (Ctrl+O), drag and drop (an overlay shows while files
   hover), or folder (Ctrl+Shift+O; every video without a subtitle file). The toolbar's one
   button is Start Queue, Pause After This Video or Resume Queue, and says why Start Queue is off
   ("Download the models first", "Nothing is waiting"); it counts full runs only. The sidebar has
   one row per video, a review run folded into its video's row, with a status mark and line
   ("Waiting · 2nd in line", "Failed at Hear the speech"), the newest ended first in Done; a click
   selects a row, ↑ and ↓ move through them, a waiting row drags to another place in line, and a
   right click opens the commands of its state: Run Next, Move Up, Move Down, Cancel, Stop
   Updating Subtitles, Check Lines, Open in Player, Show in Folder, Copy Subtitle Path, Run Again
   with Current Settings, Try Again, Remove from List. Try Again puts a failed or cancelled job
   first in line and resumes after the finished steps it kept; Run Again with Current Settings runs
   only the steps the settings saved now change, and says so when nothing changed; either starts at
   once when nothing runs, without turning the queue on, and neither puts back a video that is
   already in the list.
   One job runs at a time; GPU stages run one after another. A job takes the settings saved now
   until it first starts, and its own from then on. A full run of a video waits while a review
   run of the same video runs, and the reverse. Toasts at the bottom centre report what the window
   shows nowhere else, with a button such as Undo; errors are red.
2. **Progress.** Per job: current stage ("Settling the words"), stage progress, elapsed and
   remaining time ("about 4 min"); the finished steps with their durations, each under a plain
   title ("Listen with Whisper"). A failed job shows its stage and step ("Failed at Hear the
   speech") with the message; a failed or cancelled job shows how many finished steps it kept.
3. **Report.** When a job ends: the quality-check results, flagged lines (`UNSURE`, `NOVEL`) with
   timestamps, and the output file's path. A button opens the video in the desktop's default
   player (VLC) through the desktop portal.
4. **Review.** For each flagged line: play the clip (sound and a small picture, both from
   FFmpeg), see every engine's hypothesis, pick one or type a correction; the line is re-aligned
   and the subtitle file rewritten. Corrections never touch lines that were not flagged unless
   the owner opens them.
5. **Settings.** In a window of their own, centred over the main one when they open: models
   folder and download status, work folder and its size, engines per stage, language-model
   backend, output format, GPU check (driver, free VRAM, CUDA libraries found, FFmpeg, ffprobe,
   `claude`, the Whisper worker). Watch folders come with the
   [automation](/documentation/features/automation.md) feature.
6. **Models on first use.** Missing models are listed with their sizes and downloaded with
   progress before the first job starts.

## Data

- Settings: `~/.config/tbd-subtitles/settings.toml`.
- Jobs: the work directory of each job ([system overview](/documentation/architecture/system_overview.md#job-work-directory));
  the GUI reads `job.json`, `report.md` and the stage outputs, and listens to the runner's
  progress events.

## Design

A toolbar over two panes: the videos in a sidebar on the left, the selected job (progress, report
or review, or with no video a card to drop or add them) on the right; Settings in a second
window. The owner approved a macOS-like redesign as a clickable mockup, built in phases (see
the [roadmap](/documentation/roadmap.md#m2--desktop-gui)). Its look is in place: the mockup's
light and dark palettes (a macOS blue accent, greys, green, orange and red whose text reads at a
contrast of at least 4.5), Adwaita Sans from the system in regular, semibold and bold and Adwaita
Mono for monospace text (egui's fonts when they are missing), Phosphor icons, title 22, headline
15, body 13 and caption 11, controls 28 px high with radius 6, cards and windows with radius 10,
1 px borders and one soft shadow, and selections in a light accent tint with accent text. The
window follows the desktop's light or dark colour scheme as KDE sets it, through the desktop
portal, and switches when it changes; KDE's accent colour is not followed. Code layout follows the TBD-Reforger desktop-app pattern: one folder per feature with
`models/`, `services/` and `ui/`, where the UI draws from a borrowed view and returns events that
the application applies after the frame. Its architecture tests
(`apps/tbd_subtitles/src/tests/architecture_rules.rs`) hold the pattern here; no ticket system comes
from that project. The renderer is glow (OpenGL), because wgpu fails to create a surface on the
owner's Wayland desktop. The window runs under X11 (XWayland), where files can be dropped onto it
and a second window can be placed over it. An ignored test renders the real window offscreen at
1280 by 800, light and dark, from a copy of real work folders
(`apps/tbd_subtitles/src/application/tests/window_snapshots.rs`), so each phase of the redesign
can be compared with the mockup.

## Open work

- Milestone M2 in the [roadmap](/documentation/roadmap.md#m2--desktop-gui), which ends with the
  batch of Dressrosa 12–48 run from the window's queue.

## Decisions

- eframe over iced or Slint: the owner already uses it, it is the most active Rust GUI toolkit, and
  it draws the clip frames FFmpeg decodes
  ([clips play through FFmpeg](/documentation/decisions/desktop_gui.md#2026-09-26--clips-play-through-ffmpeg-not-libmpv)).
- The desktop portal for choosers and for opening a video
  ([the desktop portal](/documentation/decisions/desktop_gui.md#2026-09-26--the-desktop-portal-chooses-files-and-opens-videos)),
  and a review step for the owner's corrections
  ([review step](/documentation/decisions/desktop_gui.md#2026-09-26--the-owners-corrections-are-timed-by-a-review-step)).
- The desktop's colour scheme through the portal, Adwaita Sans from the system, icons from
  egui-phosphor, and X11 over Wayland
  ([the window's look](/documentation/decisions/desktop_gui.md#2026-09-28--the-window-follows-the-desktops-colour-scheme-in-adwaita-sans-under-x11)).
- A job keeps its own settings once it has started
  ([a job keeps its settings](/documentation/decisions/desktop_gui.md#2026-09-28--a-job-keeps-its-own-settings-once-it-has-started)).
- Settings in their own window
  ([Settings window](/documentation/decisions/desktop_gui.md#2026-09-28--settings-open-in-a-window-of-their-own)),
  correction runs on their video's row
  ([one row per video](/documentation/decisions/desktop_gui.md#2026-09-28--correction-runs-show-in-their-videos-row)),
  and Try Again that starts at once
  ([Try Again](/documentation/decisions/desktop_gui.md#2026-09-28--try-again-resumes-after-the-kept-steps-and-starts-at-once-when-nothing-runs)).

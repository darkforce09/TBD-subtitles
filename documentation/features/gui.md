**Status:** live

# Desktop GUI

The window the owner uses to queue videos, watch progress, read job reports and fix the lines the
pipeline was unsure about; milestone M2. What exists now: the window opens (`tbd-subtitles` or
`tbd-subtitles gui [VIDEO]...`) with two pages. Its queue panel lists the videos given on the
command line, dropped onto the window or added through the desktop's chooser (files, or a folder
for its videos without subtitles), each with a button that takes it out again. The Settings page
edits `settings.toml`, lists and downloads the models, and checks the machine. Start runs the
waiting jobs one at a time, with each step's progress and the time left; a running job can be
cancelled and an ended one retried, and the queue is kept across windows. A finished job shows
its report: whether it passes the quality check, its findings with their times, its steps, and
buttons that open the video, its folder and `report.md` through the desktop. From the report the
owner opens the line review: the flagged lines (or every line), each with its clip (the video's
sound or the voices alone, and a small picture), what every engine heard, the text and its flags.
The owner picks a reading or types the line, and Save and time again writes `review.json` and
starts a review run at once, which re-times the corrected lines and rewrites the subtitle file;
Take the correction back undoes one.

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

1. **Queue.** Add videos by file picker, drag and drop, or folder (every video without a subtitle
   file). Reorder, cancel, retry. One job runs at a time; GPU stages run one after another.
2. **Progress.** Per job: current stage, stage progress, elapsed and remaining time; the finished
   stages with their durations.
3. **Report.** When a job ends: the quality-check results, flagged lines (`UNSURE`, `NOVEL`) with
   timestamps, and the output file's path. A button opens the video in the desktop's default
   player (VLC) through the desktop portal.
4. **Review.** For each flagged line: play the clip (sound and a small picture, both from
   FFmpeg), see every engine's hypothesis, pick one or type a correction; the line is re-aligned
   and the subtitle file rewritten. Corrections never touch lines that were not flagged unless
   the owner opens them.
5. **Settings.** Models folder and download status, work folder and its size, engines per stage,
   language-model backend, output format, GPU check (driver, free VRAM, CUDA libraries found,
   FFmpeg, ffprobe, `claude`, the Whisper worker). Watch folders come with the
   [automation](/documentation/features/automation.md) feature.
6. **Models on first use.** Missing models are listed with their sizes and downloaded with
   progress before the first job starts.

## Data

- Settings: `~/.config/tbd-subtitles/settings.toml`.
- Jobs: the work directory of each job ([system overview](/documentation/architecture/system_overview.md#job-work-directory));
  the GUI reads `job.json`, `report.md` and the stage outputs, and listens to the runner's
  progress events.

## Design

A simple two-pane window: the queue on the left, the selected job (progress, report or review) on
the right. Code layout follows the TBD-Reforger desktop-app pattern: one folder per feature with
`models/`, `services/` and `ui/`, where the UI draws from a borrowed view and returns events that
the application applies after the frame. Its architecture tests
(`apps/tbd_subtitles/src/tests/architecture_rules.rs`) hold the pattern here; no ticket system comes
from that project. The renderer is glow (OpenGL), because wgpu fails to create a surface on the
owner's Wayland desktop.

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

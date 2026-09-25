**Status:** live

# Desktop GUI

The window the owner uses to queue videos, watch progress, read job reports and fix the lines the
pipeline was unsure about. Planned for milestone M2. What exists now: the window opens
(`tbd-subtitles` or `tbd-subtitles gui [VIDEO]...`), and its queue panel lists the videos given
on the command line or dropped onto the window, each with a button that takes it out again.

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
   timestamps, and the output file's path. A button opens the video in VLC.
4. **Review.** For each flagged line: play the clip (audio, and video through libmpv), see every
   engine's hypothesis, pick one or type a correction; the line is re-aligned and the subtitle
   file rewritten. Corrections never touch lines that were not flagged unless the owner opens them.
5. **Settings.** Models folder and download status, work folder and its size, engines per stage,
   language-model backend, output format, watch folders, GPU check (driver, free VRAM, CUDA
   libraries found).
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

- Milestone M2 in the [roadmap](/documentation/roadmap.md#m2--desktop-gui).

## Decisions

- eframe over iced or Slint: the owner already uses it, it is the most active Rust GUI toolkit, and
  libmpv can embed a video preview in it.

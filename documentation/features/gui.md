**Status:** live

# Desktop GUI

The window the owner uses to queue videos, watch progress, read job reports and fix the lines the
pipeline was unsure about. Planned for milestone M2; nothing is built yet.

## Where it lives

- Code (planned): `apps/tbd_subtitles/`, subcommand `gui`, built with eframe (egui).
- Entry (planned): a desktop entry "TBD Subtitles" on the host; `tbd-subtitles gui` from a terminal.
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
the application applies after the frame. Only the pattern is reused; no code or ticket system comes
from that project.

## Open work

- Milestone M2 in the [roadmap](/documentation/roadmap.md#m2--desktop-gui).

## Decisions

- eframe over iced or Slint: the owner already uses it, it is the most active Rust GUI toolkit, and
  libmpv can embed a video preview in it.

**Status:** live

# Automation

Ways to get subtitles without opening the window: a right-click entry in the file manager, watch
folders that pick up new downloads, and a command line. Planned for milestone M3; nothing is built
yet.

## Where it lives

- Code (planned): `apps/tbd_subtitles/` — the `process` subcommand, the watch service and the
  single-instance hand-off.
- Entry (planned): a Dolphin service menu file in `~/.local/share/kio/servicemenus/`, and the
  watch-folder list in the settings.
- Related: the [GUI](/documentation/features/gui.md) shows every job these start.

## Behaviour

1. **Right-click.** Selecting one or more videos in Dolphin shows "Generate subtitles". It runs
   `tbd-subtitles process --enqueue <files>`: if the app is running, the files join its queue;
   otherwise it starts minimised and processes them.
2. **Watch folders.** The owner lists folders in the settings. A new video in one is queued once
   its size has stopped changing (the download finished) and only if it has no subtitle file yet.
   Partial downloads (`.part`, `.crdownload`, `.!qB`) are ignored.
3. **Command line.** `tbd-subtitles process <file or folder>…` runs headless and prints progress;
   the exit code says whether every job passed its quality check.
4. **Single instance.** A second launch hands its files to the running app over a local socket
   and exits.
5. **Notification.** A desktop notification when a job finishes or fails.

## Data

- Watch folders and options: `~/.config/tbd-subtitles/settings.toml`.
- File events: the `notify` crate (inotify on Linux).
- Hand-off socket: in `$XDG_RUNTIME_DIR`.

## Design

No window appears for right-click or watch-folder jobs unless the owner opens it; a tray icon or
the notification leads to the report.

## Open work

- Milestone M3 in the [roadmap](/documentation/roadmap.md#m3--automation).

## Decisions

- Right-click and watch folders both feed the one job queue, so the GPU never runs two jobs at
  once.

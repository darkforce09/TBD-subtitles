**Status:** live

# Automation

Ways to get subtitles with no more than a right-click, or with nothing at all: a "Generate
subtitles" entry on a video's right-click menu in Dolphin, [watch folders](/documentation/glossary.md#watch-folder)
whose finished downloads are queued and run by themselves, one window that takes every later
launch's videos, a notification when a job ends, and a headless command line. Everything here
works while the app is open; the app starts at login only if the owner starts it.

## Where it lives

- Code, in `apps/tbd_subtitles/src/`:
  - the command line and the bare launch with videos: `apps/tbd_subtitles/src/cli/`;
  - one window per session: `apps/tbd_subtitles/src/core/single_instance.rs`;
  - the Dolphin entry: `apps/tbd_subtitles/src/core/service_menu.rs`;
  - which files are videos to take: `apps/tbd_subtitles/src/job_queue/services/video_files.rs`;
  - one scan of the watch folders: `apps/tbd_subtitles/src/job_queue/services/watch_scan.rs`,
    and its thread: `apps/tbd_subtitles/src/job_queue/services/folder_watcher.rs`;
  - every video ever queued: `apps/tbd_subtitles/src/job_queue/services/queued_history.rs`;
  - the words of a job-end notification: `apps/tbd_subtitles/src/job_queue/services/job_notice.rs`;
  - the Automation tab of Settings: `apps/tbd_subtitles/src/settings/ui/automation_tab.rs`;
  - the launch that claims the instance or hands off: `apps/tbd_subtitles/src/cli/window_command.rs`;
  - what the window does with hand-offs, watch finds and job ends:
    `apps/tbd_subtitles/src/application/actions/automation.rs`;
  - the app's icon, painted in code: `crates/app_icon/`.
- Entry: "Generate subtitles" on a video's right-click menu in Dolphin; "Open With" on a video;
  the watch folders in Settings, Automation; `tbd-subtitles process --enqueue <videos>`; and
  `tbd-subtitles process <file or folder>…` without a window.
- Related: the [desktop GUI](/documentation/features/gui.md) shows every job these start, in the
  one queue; the [AppImage runbook](/documentation/runbooks/building_the_appimage.md#steps) checks
  the Dolphin entry after a re-import.

## Behaviour

### Right-click in Dolphin

1. The owner selects one or more videos in Dolphin and right-clicks: the menu offers "Generate
   subtitles" with the app's icon.
2. It runs the AppImage as `process --enqueue` with the chosen videos. When the app is open, the
   videos join its queue and the queue starts. When it is not, the app opens its window
   minimized, with the videos queued and the queue started. A chosen folder brings its videos
   without subtitles, subfolders included. The window shows "Added N videos to the queue."

The entry is a KDE service menu that the app writes itself. Each time the app starts from its
AppImage as the first instance, it writes `~/.local/share/kio/servicemenus/tbd-subtitles.desktop`
(under `$XDG_DATA_HOME` when set), mode 0755 as KDE requires, only when its content differs: one
action, "Generate subtitles", for `video/*`, running `"<$APPIMAGE path>" process --enqueue %F`.
It copies the AppImage's icon to `~/.local/share/tbd-subtitles/tbd-subtitles.png` for the menu
to show. The entry always follows the AppImage that started last, so a re-imported AppImage takes
it over on its first start. It is always on, with no setting; Settings, Automation says
"“Generate subtitles” appears when you right-click videos in Dolphin." with the file's path,
"Appears after the app is started from its AppImage." before then, or why it could not be
written. A development build under `target/` never writes it. To remove it, delete the file; the
next start of the AppImage writes it again.

### One window

Only one copy of the app has a window. The first launch claims the instance and listens; every
later launch hands its videos to it and exits:

| Later launch | The running window |
|---|---|
| `tbd-subtitles`, `tbd-subtitles gui`, or either with videos | comes to the front and queues the videos, without starting the queue |
| `tbd-subtitles process --enqueue <videos>` | queues the videos and starts the queue, staying where it is |

Coming to the front raises and un-minimizes the window and gives it the focus; KWin may refuse
the focus, so the taskbar entry flashes too. `process` without `--enqueue`, `fix` and the
`worker` subcommands never claim the instance, so they run beside an open window. The claim
comes before the log is opened, so a later launch never empties the running window's log file.

A launch with videos and no subcommand, `tbd-subtitles a.mkv`, is the same as
`tbd-subtitles gui a.mkv`, because Gear Lever's menu entry runs the AppImage with the files and no
subcommand. The AppImage's desktop entry runs `tbd-subtitles %F` and lists the video types it
opens, so "Open With" in the file manager offers the app.

### Watch folders

1. In Settings, Automation, the owner adds folders with Add Folder… (the desktop's folder
   chooser) and takes one away with its Remove. A folder that is not there, such as one on a
   drive not mounted, stays listed, marked "Not found. Nothing in it is queued until it is
   back."
2. While the app is open, a scan every 15 seconds walks each folder and its subfolders, skipping
   hidden and symlinked folders and stopping 16 folders down. It notes each video's size and
   modification time.
3. A video is queued once two scans in a row see the same size and time, so about 15 to 30
   seconds after its download ends, when all of these hold:
   - it has no subtitle file beside it;
   - no partial download sits beside it (the same name with `.part`, `.crdownload` or `.!qB`);
   - it is not empty;
   - the app never queued it before, from any source.
4. The window shows "Added N videos from the watch folders." and the queue starts by itself (see
   [below](#the-queue-starts-by-itself)).

Videos already in a folder when it is added are queued too, when they have no subtitles; the
help under the list warns that a large folder queues every video in it without subtitles.
"Never queued before" reads the history of every video the app ever queued, by hand, from a
folder, from Dolphin or from a watch folder, so a job that failed or was removed from the list is
never queued again by a watch folder; Try Again or adding the video by hand still runs it. A
missing folder is logged once, then skipped until it is back.

### The queue starts by itself

A watch folder's find and `process --enqueue` start the queue, unless the owner pressed Pause
After This Video since the window opened; the pause is not remembered when the app starts again.
A start also runs the videos already waiting in Up Next. While models are missing the queue
cannot start: the window gives the message Start Queue gives, and a notification says so when the
window is away. The queue stays on, and its first waiting video starts once the models are on
disk.

### Working while minimized

The queue keeps going while the window is minimized, including one opened minimized by the
Dolphin entry: runner events, starting the next job, hand-offs, watch finds and notifications are
all handled whether or not the window is drawn.

### Notification when a job ends

When a full run ends while the window is away (not in front, or minimized), a desktop
notification says so and the taskbar entry flashes:

| Job | Title | Body |
|---|---|---|
| passes the quality check | Subtitles ready: Dressrosa 12 | The quality check passed. |
| needs attention | Subtitles ready: Dressrosa 12 | Quality check: <its problems>; 12 lines to check. |
| failed | Dressrosa 12 failed | At <the step>: <the error message> |

A correction run or a cancelled job gives none. The notifications share one id, so a newer one
replaces the one before. Clicking one does nothing.

### Command line

`tbd-subtitles process <file or folder>…` runs the jobs without a window, printing their
progress. A folder is searched with its subfolders for videos without subtitles. The first
failed job stops the run. The exit code says how it went:

| Exit code | Meaning |
|---|---|
| 0 | every job ran and passed its quality check |
| 2 | every job ran, and at least one failed its quality check; a summary is printed |
| 1 | an error, or a job that failed |

## Data

- Watch folders: `watch_folders` in `~/.config/tbd-subtitles/settings.toml`, a list of paths,
  saved as they change.
- Every video ever queued: `~/.local/share/tbd-subtitles/queued_videos.json`, written whole
  through a part file; a missing or broken file reads as empty.
- One window: `$XDG_RUNTIME_DIR/tbd-subtitles/instance.lock`, a file lock the first instance holds
  and the kernel frees when it exits or dies, and `$XDG_RUNTIME_DIR/tbd-subtitles/instance.sock`,
  the Unix socket it listens on. A later launch sends one JSON line, `{videos, start, raise}`, and
  waits for `{"ok":true}`.
- The Dolphin entry: `~/.local/share/kio/servicemenus/tbd-subtitles.desktop` (mode 0755) and its
  icon `~/.local/share/tbd-subtitles/tbd-subtitles.png`.
- The queue, the log and the jobs' work directories are the window's
  ([GUI data](/documentation/features/gui.md#data)).

## Design

- **No tray icon, no start at login.** Automation lives in the window; it watches while the
  window is open, in front, behind or minimized. The owner starts the app when downloads are
  expected, or leaves it open.
- **A minimized window, not a hidden one.** A right-click with no app open opens the window
  minimized, so it sits in the taskbar, where the owner finds it and its progress.
- **Notifications without actions.** A notification only tells; the window is one click away in
  the taskbar.
- **Settings, Automation** is the fifth tab: Watch folders (the list, Add Folder…, the help) and
  Right-click in Dolphin (whether the entry is there, and its path).

## Open work

- None: [M3 — Automation](/documentation/roadmap.md#m3--automation) is done, accepted by the
  owner on the host on 2026-09-29.

## Decisions

All in [automation decisions](/documentation/decisions/automation.md):

- Watching only while the app is open, with no autostart and no tray icon
  ([while the app is open](/documentation/decisions/automation.md#2026-09-29--watch-folders-work-only-while-the-app-is-open)).
- Watch folders reach into subfolders, queue the videos already there, and never queue a video
  twice
  ([recursive, and once](/documentation/decisions/automation.md#2026-09-29--watch-folders-are-recursive-queue-what-is-already-there-and-queue-each-video-once)).
- A scan every 15 seconds, not inotify
  ([periodic scan](/documentation/decisions/automation.md#2026-09-29--watch-folders-are-scanned-every-15-seconds-not-watched-with-inotify)).
- One window, by a file lock and a Unix socket
  ([one window](/documentation/decisions/automation.md#2026-09-29--one-window-per-session-by-a-file-lock-and-a-unix-socket)).
- The app writes its own Dolphin entry
  ([the Dolphin entry](/documentation/decisions/automation.md#2026-09-29--the-app-writes-its-dolphin-service-menu-when-it-starts-from-its-appimage)).
- Automation starts the queue unless it is paused
  ([the queue starts](/documentation/decisions/automation.md#2026-09-29--automation-starts-the-queue-unless-the-owner-paused-it-in-this-window)).
- Job-end notifications only while the window is away, with no click action
  ([notifications](/documentation/decisions/automation.md#2026-09-29--a-job-end-notification-comes-only-while-the-window-is-away-and-has-no-click-action)).
- The window's work goes on while it is minimized
  ([while minimized](/documentation/decisions/automation.md#2026-09-29--the-windows-work-runs-in-applogic-so-a-minimized-window-keeps-working)).
- A bare launch takes videos, and the desktop entry passes them
  ([bare launch](/documentation/decisions/automation.md#2026-09-29--a-bare-launch-takes-videos-and-the-desktop-entry-passes-f)).
- The icon is painted in its own crate
  ([the icon crate](/documentation/decisions/automation.md#2026-09-29--the-apps-icon-is-painted-in-the-app_icon-crate)).

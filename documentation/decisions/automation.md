**Status:** live

# Decisions: automation

The decisions about getting subtitles without working the window: when watch folders are
watched and how, which videos they queue, how later launches reach the one window, how the
Dolphin entry is installed, when the queue starts by itself, when a notification comes, how a
minimized window keeps working, and where the app's icon is painted. The
[decision log](/documentation/decisions/) says how entries are written; the feature is
[automation](/documentation/features/automation.md).

### 2026-09-29 — Watch folders work only while the app is open

**Context:** Watch folders could be watched by the window, by a background service started at
login, or by the window hidden in a system tray. A service or a tray means a second way to run
the app, a second place for its state, and an autostart entry to install and remove.

**Decision:** Watch folders are scanned only while the app is open, in front, behind or
minimized. The app installs no autostart entry and shows no tray icon; the owner starts it when
downloads are expected, or leaves it open. Rejected: a service at login and a tray icon, both at
the owner's word.

**Consequences:** A video that arrives while the app is closed is queued at the next start, since
the first scan finds every video without subtitles that was never queued. The acceptance test
holds "while the app is open". Closing the window stops the watching and the queue together.

**Supersedes:** none.

### 2026-09-29 — Watch folders are recursive, queue what is already there, and queue each video once

**Context:** Downloaders write into subfolders (one per series or torrent). A folder added to the
watch list may already hold videos without subtitles. A video whose job failed or that the owner
removed from the list still has no subtitles, so a watch folder would queue it again on every
scan.

**Decision:** A watch folder is walked with its subfolders, skipping hidden folders and
symlinked folders, at most 16 folders deep. Every video without a subtitle file, without a
partial-download sibling (`.part`, `.crdownload`, `.!qB`), not empty, and never queued before is
queued, including those already there when the folder is added, at the owner's word. "Queued
before" is `queued_videos.json` in the app's data folder: every video the app ever queued, from
any source, recorded for good.

**Consequences:** A failed or removed job is never queued again by a watch folder; Try Again or
adding the video by hand still runs it. Adding a large folder queues all its videos without
subtitles, which the Automation tab's help says. The history only grows; it holds one path per
video.

**Supersedes:** none.

### 2026-09-29 — Watch folders are scanned every 15 seconds, not watched with inotify

**Context:** A video is complete only once its download stops, which a file event does not say:
a downloader writes for minutes and may pause. Linux inotify watches one folder per watch, so a
recursive watch needs a watch per subfolder, re-added as subfolders appear, and runs into
`max_user_watches`; it misses changes on network and FUSE mounts. The `notify` crate wraps it and
adds a dependency.

**Decision:** A thread scans the watch folders every 15 seconds, and at once when the list
changes, sampling each video's size and modification time. A video is complete when two scans in
a row see the same sample. Rejected: inotify and the `notify` crate, since stability needs
repeated samples anyway.

**Consequences:** A finished download is queued about 15 to 30 seconds after it ends. A scan
reads folder listings and file metadata only, which costs little for folders of hundreds of
videos. A missing folder, such as one on a drive not mounted, is logged once and skipped until
it is back.

**Supersedes:** none.

### 2026-09-29 — One window per session, by a file lock and a Unix socket

**Context:** The Dolphin entry, "Open With" and a second click on the menu entry each start the
app again. Two windows would run two queues, two GPU jobs at once and two writers of the same
queue file.

**Decision:** `$XDG_RUNTIME_DIR/tbd-subtitles/instance.lock` carries an exclusive file lock held
by the first instance; `instance.sock` beside it is the Unix socket it listens on. A later launch
of the window (bare, `gui`, with videos, or `process --enqueue`) sends one JSON line
`{videos, start, raise}` and exits once answered. The claim is made before logging starts, so a
later launch never empties the first instance's log file. `process` without `--enqueue`, `fix`
and `worker` never claim.

**Consequences:** The kernel frees the lock when the first instance dies, so a crash never blocks
the next start; a claim removes a socket file a dead instance left. A plain later launch raises
the window (focus, un-minimize, and a flashing taskbar entry since KWin may refuse the focus) and
queues its videos without starting; `process --enqueue` queues and starts.

**Supersedes:** none.

### 2026-09-29 — The app writes its Dolphin service menu when it starts from its AppImage

**Context:** Dolphin's right-click entries are `.desktop` files in `kio/servicemenus/`. The
Rust-only law allows no tracked `.desktop` file, the AppImage cannot install files outside
itself, and Gear Lever keeps its copy of the AppImage under a path of its own, set at
each import.

**Decision:** On every first-instance start as an AppImage, the app writes
`~/.local/share/kio/servicemenus/tbd-subtitles.desktop` (under `$XDG_DATA_HOME` when set), mode
0755, with one action "Generate subtitles" for `video/*` that runs the AppImage named by
`$APPIMAGE` as `process --enqueue %F`, and copies the AppImage's icon to
`~/.local/share/tbd-subtitles/tbd-subtitles.png`; each only when its content differs. The entry
has no setting: it is always on. A development build under `target/` never writes it.

**Consequences:** The entry always names the AppImage that started last. Settings, Automation
shows whether it is installed and its path. Removing it means deleting the file; the next start
writes it again.

**Supersedes:** none.

### 2026-09-29 — Automation starts the queue unless the owner paused it in this window

**Context:** A video from a watch folder or from Dolphin that waits for Start Queue is not
automation. The owner may also want the queue held, for instance while using the GPU.

**Decision:** A watch folder's find and `process --enqueue` start the queue, unless the owner
pressed Pause After This Video since the window opened. The pause is not remembered across
launches. Starting the queue also runs the videos already waiting.

**Consequences:** With models missing, the queue cannot start: the window gives Start Queue's
message, and a notification says so when the window is away. A plain later launch with videos
queues them without starting, as adding by hand does.

**Supersedes:** none.

### 2026-09-29 — A job-end notification comes only while the window is away and has no click action

**Context:** Watch folders and Dolphin start jobs the owner does not watch. Fix It already shows
a notification only while the window is not in front or is minimized
([Fix It finishes visibly](/documentation/decisions/desktop_gui.md#2026-09-28--fix-it-finishes-visibly)).
A click action on a notification needs a D-Bus listener for as long as the notification lives;
the desktop portal may not route clicks to an app outside a sandbox, and KWin may refuse to raise
the window anyway.

**Decision:** When a full run finishes or fails while the window is away, a desktop notification
says so, with the flashing taskbar entry: "Subtitles ready: {name}" with "The quality check
passed." or "Quality check: {problems}; {n} lines to check.", or "{name} failed" with "At {step}:
{message}". Correction runs and cancelled jobs give none. All share one id, so a newer one
replaces the older. A notification has no click action, at the owner's word.

**Consequences:** The window is reached from the taskbar. No notification comes while the owner
looks at the window, where the sidebar says the same.

**Supersedes:** none.

### 2026-09-29 — The window's work runs in `App::logic`, so a minimized window keeps working

**Context:** eframe calls `App::ui` only for a window it draws; for a minimized window it calls
`App::logic` alone. A window opened minimized by the Dolphin entry, or minimized by the owner,
would stop starting jobs and reading hand-offs.

**Decision:** Everything the window polls runs in `App::logic`: runner events, starting the next
job, hand-offs from later launches, watch-folder finds and notifications. `App::ui` only draws.

**Consequences:** The queue runs to its end while the window is minimized. Background threads
wake the window when they have news, so `logic` runs without a frame being drawn.

**Supersedes:** none.

### 2026-09-29 — A bare launch takes videos, and the desktop entry passes `%F`

**Context:** Gear Lever's menu entry runs the AppImage with the files it is given and no
subcommand, and "Open With" in the file manager needs a desktop entry that accepts files and
names the video types.

**Decision:** `tbd-subtitles <videos>` is the same as `tbd-subtitles gui <videos>`. The AppImage's
desktop entry is `Exec=tbd-subtitles %F` with a `MimeType=` list of the video types the app
takes.

**Consequences:** "Open With" offers the app for videos and queues the chosen ones in the one
window.

**Supersedes:** none.

### 2026-09-29 — The app's icon is painted in the `app_icon` crate

**Context:** The window needs its icon as pixels at run time, and the AppImage builder needs the
same icon as a PNG. Two copies of the painting would drift, and the app cannot depend on a tool.

**Decision:** The icon is painted in code as RGBA pixels in `crates/app_icon`, a crate of layer 0
with no dependencies on the others. The window and `tools/appimage_builder` both take their pixels from it, so no
image file is tracked.

**Consequences:** The window's title bar and taskbar show the same icon as the AppImage and the
Dolphin entry. A change to the icon is one change, picked up by both at their next build.

**Supersedes:** none.

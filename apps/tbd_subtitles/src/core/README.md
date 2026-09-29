# App core

What every module of the app may use: log output to stderr, the log file and the log buffer the
log window reads, the wake threads use to reach the
window, the desktop portal, the desktop's light or dark colour scheme, how numbers are written,
the pipeline's steps as the window names them, the toasts, one window per session, the Dolphin
service menu, and the shared look and widgets of the window. It sits below the composition modules and the features, and imports none of them.

## Contents

```text
apps/tbd_subtitles/src/core/
├── background.rs       `Wake`: how a thread asks the window for a frame
├── color_scheme.rs     `Scheme` and `watch`: the desktop's light or dark preference, as it changes
├── format.rs           sizes, durations, times left, video lengths, line times, places in line, counts
├── log_buffer/         the log window's lines and model calls, and the `tracing` layers that fill them
├── logging.rs          `initialise` and `console`: the global subscriber: stderr, log file, buffer
├── mod.rs              the module tree
├── portal.rs           the desktop's chooser, opening a file, showing it in the file manager, notices
├── service_menu.rs     the Dolphin service menu "Generate subtitles", written for the running AppImage
├── single_instance.rs  one window per session: the instance lock, and later starts' hand-offs
├── steps.rs            the six stages the window shows, and each step's plain title
├── tests/              unit tests for URIs, formats, stages, scheme, toasts, logging, instance, service menu
├── toast.rs            `Toast`, `Toasts` and `ToastKind`: short messages at the bottom, with a button
└── ui/                 the palette, fonts and theme, and the widgets features draw: buttons to toasts
```

## How it works

`logging::initialise` runs once, in `cli::run` as soon as the subcommand is known (`LogRun`): it
installs a `tracing` subscriber writing to stderr without target names and with colour only when
stderr is a terminal, filtered by `RUST_LOG`, or `info` when that is unset or invalid. A worker
process writes its stderr at `DETAIL` instead (debug lines from this workspace's crates, info from
every other), with each line's target, which the job runner reads line by line; it writes each
model call to its stdout (`log_buffer::WorkerStdoutLayer`). For the window two more outputs take
`DETAIL`: the log file at `logging::window_log_path`, without colour and with each line's target
(`$XDG_STATE_HOME/tbd-subtitles/tbd-subtitles.log`, else `~/.local/state/…`, emptied at each
start, because a desktop launcher such as Gear Lever drops stderr), and the log window's buffer
(`logging::console`). A log file that cannot be opened leaves stderr alone; `RUST_LOG`, when set,
filters all three. A model call's whole exchange (its prompt, message, schema and answer, the
`model_exchange` target) never reaches stderr or the log file: those filters drop it, while the
window's buffer takes it whatever `RUST_LOG` says.

`log_buffer/` holds the newest 20,000 lines and 500 model calls of the process, each with the video
and step it belongs to, and the layers that fill them; its README says how.

`portal` talks to the XDG desktop portal over D-Bus with `ashpd` (pure Rust, zbus on async-io): it
shows the desktop's own chooser for videos, a folder or a JSON file on a thread of its own and
sends the chosen paths back on a channel; `open` asks the desktop to open a file in its default
program (VLC for the owner's videos) or a folder in the file manager, and `reveal` asks the file
manager to show a file in the folder that holds it (Show in Folder), each on a thread that sends
back whether the desktop did it (`Opened`), so the window can say when it could not. Both hand
the portal a file descriptor (`OpenFile`, `OpenDirectory`): the portal refuses `file://` URIs
for local files. The portal's answer to each request is read (`answered`): done or an "open
with" chooser the owner closed is fine, anything else is a failure with its reason. The app
starts no program for it. Every request opens a session bus connection of its own and closes it
when it ends: ashpd's shared connection holds a lock while it waits for the bus, so one request
the desktop never answered would stall every later one without a message. `file_path` turns
the chooser's `file://` URIs into paths, percent-decoded. `notify` shows a desktop notification
with a title and a body through the portal's `notification` interface, on a thread and a
connection of its own, under one id so a newer one replaces the one before; it has no answer, and
a failure is logged, since the window is away when it is sent (Fix It's finish).

`color_scheme::watch` asks the same portal's `settings` interface on a thread of its own: it
subscribes to changes of the colour scheme, sends the current one, then sends each change, waking
the window after each. KDE's colour scheme reaches it as "prefer dark", "prefer light" or "no
preference", and "no preference" is light. A desktop without the portal leaves the window light.

`format` writes the numbers every view shows: sizes as MiB or GiB, durations as `4 min 05 s`, a
time left loosely (`about 4 min`, `under 2 min`, `a few seconds`), a video's length as `25:59`
(`1:02:03` from an hour on), a line's time to the tenth of a second (`16:33.4`), places
in line (`2nd`) and counts with their noun (`2 corrections`).

`steps` groups the eighteen pipeline steps into the six stages the window shows, from "Read the
video" to "Write the subtitles", each with its title and what it does while running ("Settling
the words"), and gives every step a plain title ("Listen with Whisper") in place of its file
name; `stage_of` finds a step's stage.

`toast` holds the toasts shown now, oldest first: each has a kind (Success, Info, Working or
Error), its text, an optional button with what it does, and the time it goes away (4.2 s unless
its maker says otherwise, as the Undo toast's 6 s). At most three show; a newer one pushes the
oldest out. The application adds them in `apply`, lets them expire in `poll`, and takes one away
with its action when its button is pressed.

`single_instance` keeps one window per session. `claim` takes an exclusive `flock` on
`instance.lock` in the instance folder (`$XDG_RUNTIME_DIR/tbd-subtitles`, mode 0700, else the
app's data folder) and binds `instance.sock` beside it, after removing a socket file a dead
instance left; the kernel drops the lock with the process, so a crash leaves no stale claim. A
later start finds the lock taken (`Claim::Running`) and calls `hand_off`: it makes its videos'
paths absolute, connects (retrying every 100 ms until its patience runs out, since the first
instance takes the lock before it binds), writes one JSON line, a `HandOff` of videos, `start`
and `raise`, and reads the one-line answer. `serve` runs a thread named `single-instance` that
owns the lock and the socket, reads each connection's line (at most 1 MiB, 2 s), answers
`{"ok":true}` and sends a valid hand-off on its channel, then calls the wake once one is set;
anything else is answered `{"ok":false,"error":…}` and delivered nowhere. The thread, and with
it the lock, ends at the first connection after the channel's receiver is dropped.

`service_menu` writes the Dolphin service menu `kio/servicemenus/tbd-subtitles.desktop` under
`$XDG_DATA_HOME` or `~/.local/share`, mode 0755, as KDE requires: one action, "Generate
subtitles", for every video, that runs the AppImage as `process --enqueue %F`. `install_from_env`
does it only when `$APPIMAGE` names an existing file, and copies `$APPDIR/tbd-subtitles.png` into
the app's data folder for the menu's icon; each file is written through a `.part` file and only
when its bytes differ (`Installed::Written` or `Unchanged`). The program path in the Exec line is
double-quoted with `"`, `` ` ``, `$` and `\` backslash-escaped and `%` doubled, then escaped as a
desktop-entry string, which doubles each backslash once more, as the Desktop Entry
specification asks.

`ui` holds the look and the widgets every feature shares: the palette, the fonts and the theme
built from them, the buttons, cards, disclosures, pills and badges, progress bars and segmented
controls, switches, the icons and status marks, and the toasts' drawing. Each feature
draws its own panels with them, so the window looks the same across features.

## Public surface

- `logging::{initialise, window_log_path}`, called by `apps/tbd_subtitles/src/cli/mod.rs`.
- `background::Wake`; `portal::{choose, open, reveal, notify, Choose, Chosen, Opened}`;
  `toast::{Toast, Toasts, ToastKind, ToastId, SHOWN}`.
- `format::{size, duration, about, length, clock_tenths, ordinal, plural}`.
- `steps::{STAGES, Stage, stage_of, step_title}`.
- `color_scheme::{Scheme, watch}`.
- `single_instance::{HandOff, Claim, Instance, instance_dir, claim, serve, hand_off}`.
- `service_menu::{MENU_FILE, Installed, menu_dir, menu_text, install_into, install_from_env}`.
- `ui::theme::{install, follow}`, `ui::fonts`, `ui::palette::palette`, the shared colours, and
  the widgets `ui::{button, card, disclosure, icons, pill, progress, segmented, switch, toast}`.

## Boundaries

- Depends on: `job_model::StepName` in `steps.rs`; `tracing-subscriber` in `logging.rs`; `ashpd`
  and `pollster` in `portal.rs` and `color_scheme.rs`, with `futures-util` for the change stream;
  `eframe::egui` and `egui-phosphor` in `ui/`; `serde_json` and
  `inference::model_store::app_data_dir` in `single_instance.rs` and `service_menu.rs`.
- Used by: `apps/tbd_subtitles/src/main.rs`; `crate::application`; the features' `ui` and
  `services`.
- Rules: `core` imports no feature and neither `application` nor `cli`
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a chosen `file://` URI becomes its video path
  (`a_chosen_video_uri_becomes_its_path` in `tests/portal.rs`); a portal refusal is a failure
  (`the_portal_answer_decides_whether_an_open_failed`); only a dark
  preference makes the window dark (`only_a_dark_preference_makes_the_window_dark` in
  `tests/color_scheme.rs`); every step belongs to exactly one stage, in run order
  (`every_step_belongs_to_exactly_one_stage_in_order` in `tests/steps.rs`); at most three toasts
  show and each goes in its time (`toasts_expire_in_their_time_and_the_oldest_leaves_first` in
  `tests/toast.rs`); a second claim finds the first running
  (`a_second_claim_finds_the_first_running_until_it_is_dropped` in `tests/single_instance.rs`)
  and only a valid hand-off reaches the window
  (`a_malformed_line_is_refused_and_delivers_nothing`); the service menu's program path stays one
  quoted argument (`a_path_with_spaces_stays_one_quoted_argument` in `tests/service_menu.rs`) and
  an unchanged menu is not rewritten
  (`an_unchanged_menu_is_not_rewritten_and_a_moved_appimage_is`).

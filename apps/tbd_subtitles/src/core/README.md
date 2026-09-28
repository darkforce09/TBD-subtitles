# App core

What every module of the app may use: log output to stderr, the wake threads use to reach the
window, the desktop portal, the desktop's light or dark colour scheme, how numbers are written,
the pipeline's steps as the window names them, the toasts, and the shared look and widgets of the
window. It sits below the composition modules and the features, and imports none of them.

## Contents

```text
apps/tbd_subtitles/src/core/
├── background.rs    `Wake`: how a thread asks the window for a frame
├── color_scheme.rs  `Scheme` and `watch`: the desktop's light or dark preference, followed as it changes
├── format.rs        sizes, durations, rough times left, video clocks, places in line and counts
├── logging.rs       `initialise`: the global log subscriber, filtered by `RUST_LOG`, writing to stderr
├── mod.rs           the module tree
├── portal.rs        the desktop's chooser, opening a file in its program, showing it in the file manager
├── steps.rs         the six stages the window shows, and each step's plain title
├── tests/           unit tests for the portal's file URIs, the formats, the stages, the scheme, toasts
├── toast.rs         `Toast`, `Toasts` and `ToastKind`: short messages at the bottom, with a button
└── ui/              the palette, fonts and theme, and the buttons, icons and toasts features draw
```

## How it works

`logging::initialise` runs once, first thing in `main`: it installs a `tracing` subscriber whose
filter comes from `RUST_LOG`, or `info` when that is unset or invalid, writing to stderr without
target names and with colour only when stderr is a terminal.

`portal` talks to the XDG desktop portal over D-Bus with `ashpd` (pure Rust, zbus on async-io): it
shows the desktop's own chooser for videos, a folder or a JSON file on a thread of its own and
sends the chosen paths back on a channel; `open` asks the desktop to open a file in its default
program (VLC for the owner's videos) or a folder in the file manager, and `reveal` asks the file
manager to show a file in the folder that holds it (Show in Folder). The app starts no program
for it. `file_uri` and `file_path` turn paths into `file://` URIs and back, percent-encoded.

`color_scheme::watch` asks the same portal's `settings` interface on a thread of its own: it
subscribes to changes of the colour scheme, sends the current one, then sends each change, waking
the window after each. KDE's colour scheme reaches it as "prefer dark", "prefer light" or "no
preference", and "no preference" is light. A desktop without the portal leaves the window light.

`format` writes the numbers every view shows: sizes as MiB or GiB, durations as `4 min 05 s`, a
time left loosely (`about 4 min`, `under 2 min`, `a few seconds`), video times as `h:mm:ss` or
`h:mm:ss.d`, places in line (`2nd`) and counts with their noun (`2 corrections`).

`steps` groups the eighteen pipeline steps into the six stages the window shows, from "Read the
video" to "Write the subtitles", each with its title and what it does while running ("Settling
the words"), and gives every step a plain title ("Listen with Whisper") in place of its file
name; `stage_of` finds a step's stage.

`toast` holds the toasts shown now, oldest first: each has a kind (Success, Info, Working or
Error), its text, an optional button with what it does, and the time it goes away (4.2 s unless
its maker says otherwise, as the Undo toast's 6 s). At most three show; a newer one pushes the
oldest out. The application adds them in `apply`, lets them expire in `poll`, and takes one away
with its action when its button is pressed.

`ui` holds the look and the widgets every feature shares: the palette, the fonts and the theme
built from them, the buttons, the icons and status marks, and the toasts' drawing. Each feature
draws its own panels with them, so the window looks the same across features.

## Public surface

- `logging::initialise`, called by `apps/tbd_subtitles/src/main.rs`.
- `background::Wake`; `portal::{choose, open, reveal, Choose, Chosen}`;
  `toast::{Toast, Toasts, ToastKind, ToastId, SHOWN}`.
- `format::{size, duration, about, clock, clock_tenths, ordinal, plural}`.
- `steps::{STAGES, Stage, stage_of, step_title}`.
- `color_scheme::{Scheme, watch}`.
- `ui::theme::{install, follow}`, `ui::fonts`, `ui::palette::palette`, the shared colours, and
  the widgets `ui::{button, icons, toast}`.

## Boundaries

- Depends on: `job_model::StepName` in `steps.rs`; `tracing-subscriber` in `logging.rs`; `ashpd`
  and `pollster` in `portal.rs` and `color_scheme.rs`, with `futures-util` for the change stream;
  `eframe::egui` and `egui-phosphor` in `ui/`.
- Used by: `apps/tbd_subtitles/src/main.rs`; `crate::application`; the features' `ui` and
  `services`.
- Rules: `core` imports no feature and neither `application` nor `cli`
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a video path survives the trip through a
  `file://` URI (`a_video_path_becomes_a_file_uri_and_back` in `tests/portal.rs`); only a dark
  preference makes the window dark (`only_a_dark_preference_makes_the_window_dark` in
  `tests/color_scheme.rs`); every step belongs to exactly one stage, in run order
  (`every_step_belongs_to_exactly_one_stage_in_order` in `tests/steps.rs`); at most three toasts
  show and each goes in its time (`toasts_expire_in_their_time_and_the_oldest_leaves_first` in
  `tests/toast.rs`).

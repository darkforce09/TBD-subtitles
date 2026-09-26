# App core

What every module of the app may use: log output to stderr, the wake threads use to reach the
window, the desktop portal, how sizes are written, and the shared look of the window. It sits below
the composition modules and the features, and imports none of them.

## Contents

```text
apps/tbd_subtitles/src/core/
├── background.rs  `Wake`: how a thread asks the window for a frame
├── format.rs      `size` and `duration`: bytes as MiB or GiB, seconds as hours, minutes, seconds
├── logging.rs     `initialise`: the global log subscriber, filtered by `RUST_LOG`, writing to stderr
├── mod.rs         the module tree
├── portal.rs      the desktop's file and folder chooser, and opening a file in its default program
├── tests/         unit tests for the portal's file URIs and the size format
└── ui/            the colours and sizes every feature's UI shares
```

## How it works

`logging::initialise` runs once, first thing in `main`: it installs a `tracing` subscriber whose
filter comes from `RUST_LOG`, or `info` when that is unset or invalid, writing to stderr without
target names and with colour only when stderr is a terminal.

`portal` talks to the XDG desktop portal over D-Bus with `ashpd` (pure Rust, zbus on async-io): it
shows the desktop's own chooser for videos, a folder or a JSON file on a thread of its own and
sends the chosen paths back on a channel; `open` asks the desktop to open a file in its default
program (VLC for the owner's videos) or a folder in the file manager. The app starts no program
for it. `file_uri` and `file_path` turn paths into `file://` URIs and back, percent-encoded.

`ui` holds constants, not widgets: each feature draws its own panels and takes the shared colours
from here, so the window looks the same across features.

## Public surface

- `logging::initialise`, called by `apps/tbd_subtitles/src/main.rs`.
- `background::Wake`; `portal::{choose, open, Choose, Chosen}`; `format::{size, duration}`.
- `ui::{MUTED_TEXT, GOOD, CAUTION, BAD}`, the shared colours.

## Boundaries

- Depends on: `tracing-subscriber` in `logging.rs`; `ashpd` and `pollster` in `portal.rs`;
  `eframe::egui::Color32` in `ui/`.
- Used by: `apps/tbd_subtitles/src/main.rs`; `crate::application`; the features' `ui` and
  `services`.
- Rules: `core` imports no feature and neither `application` nor `cli`
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a video path survives the trip through a
  `file://` URI (`a_video_path_becomes_a_file_uri_and_back` in `tests/portal.rs`).

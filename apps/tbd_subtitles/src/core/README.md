# App core

What every module of the app may use: log output to stderr and the shared look of the window. It
sits below the composition modules and the features, and imports none of them.

## Contents

```text
apps/tbd_subtitles/src/core/
├── logging.rs  `initialise`: the global log subscriber, filtered by `RUST_LOG`, writing to stderr
├── mod.rs      the module tree
└── ui/         the colours and sizes every feature's UI shares
```

## How it works

`logging::initialise` runs once, first thing in `main`: it installs a `tracing` subscriber whose
filter comes from `RUST_LOG`, or `info` when that is unset or invalid, writing to stderr without
target names and with colour only when stderr is a terminal. Modules then log with the `tracing`
macros; the window logs each queue change.

`ui` holds constants, not widgets: each feature draws its own panels and takes the shared colours
from here, so the window looks the same across features.

## Public surface

- `logging::initialise`, called by `apps/tbd_subtitles/src/main.rs`.
- `ui::MUTED_TEXT`, the colour of secondary text, used by the window and the queue panel.

## Boundaries

- Depends on: `tracing-subscriber` in `logging.rs`; `eframe::egui::Color32` in `ui/`.
- Used by: `apps/tbd_subtitles/src/main.rs`; `crate::application::window`;
  `crate::job_queue::ui::queue_panel`.
- Rules: `core` imports no feature and neither `application` nor `cli`
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); the subscriber is installed once, before
  any other work.

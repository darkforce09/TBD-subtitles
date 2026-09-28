# Log console

The feature for the log window: every line the app, its jobs and the programs they start log while
the window runs, in a second window opened from the toolbar's log button or Ctrl+L. The lines are
filtered by level and text, follow the newest, and can be copied or cleared; Open Log File opens
the whole log in the desktop's text editor.

## Contents

```text
apps/tbd_subtitles/src/log_console/
├── events.rs  `LogConsoleEvent`: a level, a search, Clear, Open Log File
├── mod.rs     the module tree and the feature's header
├── models/    the window's copy of the lines, the filter, the shown lines; no rendering code
├── services/  a line's columns as the window writes them, and the text Copy hands over
└── ui/        the log window's content: the filter bar, the lines, the footer
```

## How it works

The folder follows the layout every feature shares: `models/` and `services/` hold data and logic
free of egui, and `ui/` draws from a borrow the application lends it and returns events for the
application to apply after the frame. The lines come from the process's log buffer
(`crate::core::log_buffer`), which a `tracing` layer fills with every event of the window's run:
the application's own lines, each job's steps (`job_queue::services::progress_log`), Fix It's
passes, what the pipeline decides, each `claude` call's summary, and every external program's
start, stderr lines and end (`crates/child_process/`). While the window is open the application
copies the lines logged since its last read into `models::console::Console` each frame, so none
is lost while it is closed; the buffer and the console both keep the newest 20,000 lines, and the
log file keeps them all.

## Public surface

- `events::LogConsoleEvent`, `models::console::Console` and `ui::console_window_ui`, for the
  application.

## Boundaries

- Depends on: `crate::core` (the log buffer, the look, `format::plural`), `tracing` (`Level`)
  and `eframe` (in `ui/` only).
- Used by: `crate::application`.
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`
  (`module_roots_and_documentation_describe_the_entire_source_tree`); `models/` and `services/`
  never name egui or eframe, and the feature never imports `application`, `cli` or another
  feature's `ui` (`dependency_boundaries_and_external_test_placement_are_enforced`); both tests
  are in `apps/tbd_subtitles/src/tests/architecture_rules.rs`.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md#the-log-window) — the log window as the owner sees
  it.
- [The log window](/documentation/decisions/desktop_gui.md#2026-09-28--a-log-window-shows-everything-the-app-does)
  — why one `tracing` buffer feeds it and child stderr is streamed.

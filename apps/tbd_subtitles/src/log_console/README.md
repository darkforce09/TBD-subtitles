# Log console

The feature for the log window: every line the app, its jobs and the programs they start log while
the window runs, grouped under the video and step they belong to and marked with who wrote them,
and every language-model call with what was sent and what came back, in a second window opened
from the toolbar's log button or Ctrl+L. Lines are filtered by level, writer and text; a click
shows a line whole; Open Log File opens the whole log in the desktop's text editor.

## Contents

```text
apps/tbd_subtitles/src/log_console/
├── events.rs  `LogConsoleEvent`: view, level, writer, search, open line or call, Clear, log file
├── mod.rs     the module tree and the feature's header
├── models/    both views' state: lines with filters and groups, calls, who wrote a line
├── services/  the time, headers, lines and calls in words, and what Copy hands over
└── ui/        the bar and filters, the activity list and its detail panel, the Model Calls view
```

## How it works

The folder follows the layout every feature shares: `models/` and `services/` hold data and logic
free of egui, and `ui/` draws from a borrow the application lends it and returns events for the
application to apply after the frame. The lines come from the process's log buffer
(`crate::core::log_buffer`), which a `tracing` layer fills with every event of the window's run:
the application's own lines, each job's steps (`job_queue::services::progress_log`), Fix It's
passes, what the pipeline decides, each `claude` call's summary, and every external program's
start, stderr lines and end (`crates/child_process/`), each with the video and step it belongs to;
and it keeps every model call whole (`crates/inference/src/llm/call_log/`), a worker's through its
stdout. While the window is open the application copies the lines and calls logged since its last
read into `models::console::LogConsole` each frame, so none is lost while it is closed; the
buffer and the console keep the newest 20,000 lines and 500 calls, and the log file keeps every
line, never a call's prompt or answer.

## Public surface

- `events::LogConsoleEvent`, `models::console::{LogConsole, ConsoleView}` and
  `ui::console_window_ui`, for the application.

## Boundaries

- Depends on: `crate::core` (the log buffer, the steps' titles, the look, `format::plural`),
  `job_model::StepName`, `tracing` (`Level`) and `eframe` (in `ui/` only).
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
- [Model calls in the log window](/documentation/decisions/desktop_gui.md#2026-09-28--model-calls-show-in-the-log-window-never-in-the-log-file)
  — why calls stay in memory and how they leave a worker.

# Log window

The log window's content, drawn from the borrowed `LogConsole`: the bar, the filters, the activity
list with its detail panel, and the Model Calls view. It returns events and changes nothing,
except that Copy puts text on the clipboard itself.

## Contents

```text
apps/tbd_subtitles/src/log_console/ui/
├── activity_view.rs   the activity list: group headers, and one row per line with its writer's chip
├── calls_view.rs      the Model Calls view: the calls on the left, the open call on the right
├── console_window.rs  `console_window_ui`: the bar, the filters, the view shown, the footer
├── line_detail.rs     the detail panel under the list: the open line whole, and Show Model Call
└── mod.rs             the module list and the entry point
```

## How it works

The bar on the toolbar grey switches between Activity and Model Calls (with the count of calls
kept) and holds Copy, Clear and Open Log File (off when the window writes no log file). Under it,
the activity view shows its levels (Errors and Warnings counted, Info, Debug) and its writers
(Everyone, App, Jobs, AI, Programs), and both views the search.

The activity list draws only the rows in view, each one row high and never scrolling sideways: a
header in semibold over a hairline names the video and the step in words; a line shows its time,
a chip for who wrote it (the AI blue, a job green, a program grey, the app outlined), a warning
mark in orange or red, and its message in the monospace font, cut with `…` at the width; debug
lines are grey. A click opens the line in a resizable panel below: its level, writer, source and
time, its video and step, the message whole, wrapped and selectable, Copy, and Show Model Call
when the line sums up a call; ✕ or Esc closes it. The list sticks to the newest row until the
owner scrolls up.

The Model Calls view lists the calls on the left, each with its purpose (red, with a mark, when
it failed) over its time, place, model and seconds. The open call shows its purpose, place,
model, id, seconds, tokens and cost, and why it failed; then Answer (open at first; "What claude
printed" for a failed call), Message, System prompt and Schema, each foldable with its size, a
Copy button and the text whole in a well. Copy in the bar copies the whole call.

## Boundaries

- Depends on: `crate::log_console::{events, models, services}`, `crate::core::log_buffer`,
  `crate::core::ui` (palette, fonts, buttons, icons, the segmented control, the disclosure),
  `crate::core::format::plural` and `eframe`.
- Used by: `crate::application` (`feature_views::log_console_ui`, inside the log window's
  viewport).
- Rules: only the application calls it
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

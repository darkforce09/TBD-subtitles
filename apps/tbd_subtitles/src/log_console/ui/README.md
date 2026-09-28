# Log window

The log window's content, drawn from the borrowed `Console`: the filter bar, the lines, and the
footer. It returns events and changes nothing, except that Copy puts the shown lines on the
clipboard itself.

## Contents

```text
apps/tbd_subtitles/src/log_console/ui/
├── console_window.rs  `console_window_ui`: the filter bar, the scrolling lines, the footer
└── mod.rs             the module list and the entry point
```

## How it works

The filter bar on the toolbar grey holds the levels as a segmented control (Errors and Warnings
with their counts, Info, Debug), the search field, and on the right Copy, Clear and Open Log File
(off when the window writes no log file). The lines scroll both ways in the window's monospace
font: the time and the level's colour first (red errors, orange warnings, blue info, grey debug),
then the source and the message, an error's message red too. Only the rows in view are laid out,
rows never wrap, and each row's text can be selected. The list sticks to the newest line until
the owner scrolls up, and again once they scroll back to the end. With nothing to show it says
"Nothing is logged yet." or "No line matches the filter."; the footer counts the lines shown of
those kept and says the times count from the window's start.

## Boundaries

- Depends on: `crate::log_console::{events, models, services}`, `crate::core::ui` (palette,
  buttons, icons, the segmented control), `crate::core::format::plural` and `eframe`.
- Used by: `crate::application` (`feature_views::log_console_ui`, inside the log window's
  viewport).
- Rules: only the application calls it
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

# Log console services

The log window's words, with no rendering code: a line's time, level and source in fixed-width
columns, and the shown lines as the text Copy puts on the clipboard.

## Contents

```text
apps/tbd_subtitles/src/log_console/services/
├── console_text.rs  `time`, `level`, `source`, `line_text` and `copy_text`
├── mod.rs           the module list
└── tests/           unit tests of each column and of Copy's text
```

## How it works

`time` writes the time since the window opened as `01:23.456`, or `1:02:03.4` from the first hour
on, nine characters wide. `level` pads the level's name to five characters. `source` keeps a
target's last name (`pipeline::workers` becomes `workers`), cut or padded to fourteen. In the
window's monospace font the three columns line up; `line_text` joins them with the message, and
`copy_text` writes every shown line, one per text line.

## Boundaries

- Depends on: `crate::core::log_buffer::LogLine`, `crate::log_console::models::console` and
  `tracing` (`Level`).
- Used by: `crate::log_console::ui`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

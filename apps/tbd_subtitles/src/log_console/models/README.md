# Log console models

The log window's state, with no rendering code: its copy of the logged lines, the level and text
it filters by, the lines that pass, and the count of errors and warnings.

## Contents

```text
apps/tbd_subtitles/src/log_console/models/
├── console.rs  `Console` and `ConsoleFilter`: the kept lines, the filter, the shown lines, counts
├── mod.rs      the module list
└── tests/      unit tests of appending, the level and search filters, Clear and the capacity
```

## How it works

`Console::append` takes the lines read from the log buffer, remembers the next sequence number to
read, and drops the oldest past 20,000 (`CAPACITY`). `ConsoleFilter` lets a line through when its
level is the chosen one or more severe (Debug shows everything) and, when a search is typed, when
its source or message holds the search, ignoring case and the spaces around it. The positions of
the lines that pass are kept, refreshed only for new lines or when the filter changes, so the
window lays out only the rows in view. `problems` counts the errors and warnings kept, which the
level buttons show. `clear` forgets the lines but keeps the next number, so later lines still
arrive.

## Boundaries

- Depends on: `crate::core::log_buffer` (`LogLine`, `CAPACITY`) and `tracing` (`Level`).
- Used by: `crate::log_console::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

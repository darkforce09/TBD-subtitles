# Log console models

The log window's state, with no rendering code: which view shows and the search both views share,
the activity list with its filters, groups and open line, the Model Calls list with its open call,
and who wrote each line.

## Contents

```text
apps/tbd_subtitles/src/log_console/models/
├── activity.rs  `Activity`, `ActivityFilter` and `RowView`: lines, filters, group headers, open line
├── calls.rs     `Calls`: the kept model calls, the ones the search lets through, and the open one
├── console.rs   `LogConsole` and `ConsoleView`: the view shown, the shared search, both lists
├── mod.rs       the module list
├── tests/       unit tests of the groups, filters, selections, search, clearing and the caps
└── who.rs       `Who`: App, Job, AI or Program, found from a line's target
```

## How it works

`Activity::append` takes the lines read from the log buffer, remembers the next number to read,
and drops the oldest past 20,000. A line passes the filter when its level is the chosen one or
more severe (Debug shows everything), its writer is the chosen one (or Everyone), and, when a
search is typed, its text, source, video or step holds it, ignoring case. The rows are the lines
that pass, with a header before the first line of each new (video, step): a job's lines, its
workers' and its programs' lines stay under the step they belong to, and a line about no video or
step (the owner's actions) never starts or breaks a group. Rows are rebuilt only for new lines or
a changed filter, so the list lays out only the rows in view. `problems` counts the errors and
warnings kept; `select` opens a line in the detail panel, which stays open while the line is kept.

`Who::of` reads a line's target: the language model's backend and Fix It are the AI, the pipeline,
the stages and the job runner's lines are a Job, `child_process` is a Program, and everything else
is the App.

`Calls` keeps the newest 500 model calls with their video and step. Its search matches a call's
model, purpose, id, video or step, never its prompt or answer, which are too long to search as the
owner types; the open call stays open while it is kept. `LogConsole` holds both lists, the view
shown and the search as typed, and filters both lists with it, trimmed and lower-cased;
`show_call` opens the Model Calls view on a call, from the line that sums it up.

## Boundaries

- Depends on: `crate::core::log_buffer` (`LogLine`, `KeptCall`, `CAPACITY`, `CALL_CAPACITY`) and
  `tracing` (`Level`).
- Used by: `crate::log_console::{services, ui, events}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); app lines never break a group
  (`a_header_starts_each_new_video_and_step_and_app_lines_never_break_a_group` in
  `tests/activity.rs`); a search never reads a prompt
  (`a_search_matches_the_purpose_or_the_step_but_never_the_prompt` in `tests/calls.rs`).

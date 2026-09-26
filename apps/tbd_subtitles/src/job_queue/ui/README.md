# Job queue panels

The queue panel on the left and the selected job's progress on the right, drawn from a borrowed
view; they return events and change nothing.

## Contents

```text
apps/tbd_subtitles/src/job_queue/ui/
├── mod.rs            the module list and the two entry points
├── progress_view.rs  the selected job: clock, time left, share done, and a row per step
└── queue_panel.rs    add buttons, Start and Pause, and a row per job with its buttons
```

## How it works

`queue_panel_ui` draws Add videos and Add folder, Start (enabled with a waiting job and every
model on disk) or Pause, the counts of waiting and done jobs, a warning while a model is missing,
then a row per job: its mark (waiting, running, passed, finished with findings that fail the
quality check, failed, cancelled), its file name (with "· corrections" for a review run), and
under it the running job's bar, time left and Cancel, or a waiting job's Up, Down, Next and remove
buttons, or an ended job's Retry and remove.
`progress_view_ui` draws the selected job; while it runs, its elapsed time, time left and a row
per step.

## Boundaries

- Depends on: `crate::job_queue::{events, models, services::time_left}`; `crate::core::{format,
  ui}`; `eframe`.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a running job offers only Cancel (the
  header of `queue_panel.rs`).

# Job queue models

The data the queue panel draws, with no rendering code: the borrowed, read-only view of the queue
that the application lends the panel for one frame.

## Contents

```text
apps/tbd_subtitles/src/job_queue/models/
├── mod.rs   the module tree
└── view.rs  `JobQueueView`: the queued videos in run order, borrowed for one frame
```

## Boundaries

- Depends on: the standard library (`PathBuf`).
- Used by: `crate::job_queue::ui::queue_panel`, which draws from the view, and
  `crate::application::feature_views`, which builds it from the queue each frame.
- Rules: the view borrows and never owns or changes the queue, and nothing here names egui or
  eframe (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

# Job queue services

The edits of the queue, with no rendering code: adding videos and taking one out. The application
calls them when it applies its actions after a frame.

## Contents

```text
apps/tbd_subtitles/src/job_queue/services/
├── mod.rs            the module tree
├── queue_editing.rs  `add_videos` and `remove_video` over the queue of video paths
└── tests/            unit tests for adding and removing
```

## Boundaries

- Depends on: the standard library (`PathBuf`).
- Used by: `crate::application`, whose `apply` calls both functions.
- Rules:
  - `add_videos` appends each video not already queued, skips empty paths, keeps the given order
    and returns how many it added (`adding_skips_duplicates_and_empty_paths_and_keeps_order` in
    `tests/queue_editing.rs`);
  - `remove_video` returns the removed path, and an index past the end changes nothing
    (`removing_past_the_end_changes_nothing` in `tests/queue_editing.rs`);
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

# Job queue panel

The queue panel drawn on the left of the window: a "Queue" heading, then either a hint to drop
videos onto the window or the queued videos by file name, each with a button that takes it out.
It draws from a borrowed view and returns events; it changes nothing.

## Contents

```text
apps/tbd_subtitles/src/job_queue/ui/
├── mod.rs          the module tree and the re-export of `queue_panel_ui`
└── queue_panel.rs  `queue_panel_ui`: the heading, the empty hint or the scrolling list of videos
```

## Boundaries

- Depends on: `crate::job_queue::models::view::JobQueueView`;
  `crate::job_queue::events::JobQueueEvent`; `crate::core::ui::MUTED_TEXT`; `eframe::egui`.
- Used by: `crate::application::feature_views`, which draws the panel each frame.
- Rules:
  - the panel only reads its view and pushes `JobQueueEvent`s; the application applies them after
    the frame (`actions_change_the_queue_only_when_applied` in
    `apps/tbd_subtitles/src/application/tests/rendering.rs`);
  - an empty queue says how to add videos, and a video shows by its file name with the full path
    on hover (`an_empty_queue_says_how_to_add_videos`, `queued_videos_show_by_file_name` in the
    same file);
  - no module outside the feature but `application` imports this folder
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

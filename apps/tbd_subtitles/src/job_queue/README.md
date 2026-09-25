# Job queue

The feature that holds the videos waiting for subtitles: the queue panel on the left of the
window, the borrowed view it draws from, the edits that add and remove videos, and the events the
panel returns. Each job's progress belongs here too; that part is not written yet.

## Contents

```text
apps/tbd_subtitles/src/job_queue/
├── events.rs  `JobQueueEvent`: what the panel asks for, removing a video by its queue position
├── mod.rs     the module tree
├── models/    `JobQueueView`, the queue as the panel sees it for one frame
├── services/  `add_videos` and `remove_video`, the only edits of the queue
└── ui/        `queue_panel_ui`: the queued videos by file name, each with a remove button
```

## How it works

The queue itself, a `Vec<PathBuf>` in run order, lives in the application state; this feature
owns how it is shown and changed, not where it is kept.

```text
application ──lends──▶ JobQueueView { videos: &[PathBuf] }       (models/)
                           │
                           ▼
                     queue_panel_ui ──push──▶ JobQueueEvent::Remove(index)   (ui/, events.rs)
                                                   │
application ◀──────── Action::RemoveFromQueue ◀────┘
     │
     └──apply──▶ queue_editing::remove_video / add_videos   (services/)
```

A dropped or named video enters through `add_videos`, which skips empty paths and videos already
queued and keeps the given order. `remove_video` ignores an index past the end, so a stale event
changes nothing. The panel never edits the queue: it only pushes events.

## Public surface

- `models::view::JobQueueView`, which the application builds from its queue each frame.
- `ui::queue_panel_ui`, which the application draws in the left panel.
- `events::JobQueueEvent`, which the application converts into its `Action`.
- `services::queue_editing::{add_videos, remove_video}`, which the application applies actions
  with.

## Boundaries

- Depends on: `crate::core::ui::MUTED_TEXT` in `ui/`; `eframe::egui` in `ui/` only; the standard
  library elsewhere.
- Used by: `crate::application` (`mod.rs`, `events.rs`, `feature_views.rs`).
- Rules:
  - `models/` and `services/` never name egui or eframe, and the feature imports neither
    `application` nor `cli` (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - adding skips duplicates and empty paths and keeps order, and removing past the end changes
    nothing (`adding_skips_duplicates_and_empty_paths_and_keeps_order`,
    `removing_past_the_end_changes_nothing` in `services/tests/queue_editing.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue's intended behaviour: add, reorder,
  cancel, retry, one job at a time.

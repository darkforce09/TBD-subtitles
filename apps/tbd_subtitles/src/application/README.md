# Application window

The eframe desktop window: the application state, one frame of drawing, and the actions that
change the state. It composes the feature modules and is the only module that draws them.

## Contents

```text
apps/tbd_subtitles/src/application/
├── events.rs         `Action`: queue videos or remove one; built from a `JobQueueEvent`
├── feature_views.rs  lends the queue panel its borrowed view and turns its events into actions
├── mod.rs            `TbdSubtitlesApp`, its queue and `apply`; `launch`, which opens the window
├── tests/            headless rendering tests of the frame and of applying actions
└── window.rs         one frame: dropped files, the queue panel on the left, the central panel
```

## How it works

`launch` opens a native window titled "TBD Subtitles" (application id `tbd-subtitles`), 1100 by
700 and at least 640 by 400, with drag and drop on and the glow renderer, and returns when it
closes. `TbdSubtitlesApp` holds the queue of videos in run order; the videos passed to `launch`
enter it as the first `Action`.

Each frame runs in two steps:

```text
frame_ui(&self)
  ├── dropped files ──▶ Action::QueueVideos
  ├── feature_views::queue_ui ──▶ JobQueueView ──▶ queue_panel_ui
  │                                                └──▶ JobQueueEvent ──▶ Action::RemoveFromQueue
  └── central panel: the title and a note that no stage is built

apply(&mut self, actions)
  └── queue_editing::add_videos / remove_video, then a log line
```

`frame_ui` borrows the state immutably and only collects actions; `apply` is the one place the
state changes, after the frame, in the order the actions were asked for. The shared muted text
colour comes from `core::ui`.

## Boundaries

- Depends on: `crate::job_queue` (`events::JobQueueEvent`, `models::view::JobQueueView`,
  `ui::queue_panel_ui`, `services::queue_editing`); `crate::core::ui::MUTED_TEXT`; `eframe`,
  `anyhow` and `tracing`.
- Used by: `crate::cli`, which calls `launch` for the `gui` subcommand and for no subcommand.
- Rules:
  - nothing changes state while a frame is drawn: every change is an `Action` applied after the
    frame (`actions_change_the_queue_only_when_applied` in `tests/rendering.rs`);
  - an empty queue tells the user how to add videos, and queued videos show by file name
    (`an_empty_queue_says_how_to_add_videos`, `queued_videos_show_by_file_name` in
    `tests/rendering.rs`);
  - no feature imports this module
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the two-pane window and the borrowed-view
  pattern it follows.

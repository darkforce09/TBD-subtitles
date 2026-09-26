# Application window

The eframe desktop window: the application state, one frame of drawing, the actions that change
the state, and the threads it waits on. It composes the feature modules and is the only module
that draws them.

## Contents

```text
apps/tbd_subtitles/src/application/
├── actions/          applying each feature's actions and folding its threads' answers in
├── background.rs     `Pending`: the threads the window waits on, and the chooser's answers
├── environment.rs    `Environment`: the settings file, the runtime folder, the wake; scratch in tests
├── events.rs         `Action` and `Page`, built from the features' events
├── feature_views.rs  lends each feature its borrowed view and turns its events into actions
├── mod.rs            `TbdSubtitlesApp`, `apply`, and `launch`, which opens the window
├── tests/            headless rendering tests of the frame and of applying actions
└── window.rs         one frame: page tabs, the queue on the left, the page on the right
```

## How it works

`launch` opens a native window titled "TBD Subtitles" (application id `tbd-subtitles`), 1200 by
760 and at least 760 by 480, with drag and drop on and the glow renderer, and returns when it
closes. Its `Environment` names the owner's settings file and runtime folder and wakes the window
from any thread (`request_repaint`); the tests build one over a scratch folder. `TbdSubtitlesApp`
holds the page shown (Jobs or Settings), the queue of videos in run order, the settings page and
`Pending`, the receiving end of every thread it started. The videos passed to `launch` enter the
queue as the first `Action`; the machine checks and the work folder's measure start at once.

Each frame runs in three steps:

```text
poll(&mut self)        the threads' answers: chooser paths, download progress, checks, sizes
frame_ui(&self)
  ├── dropped files ──▶ Action::QueueVideos
  ├── page tabs ──▶ Action::ShowPage
  ├── feature_views::queue_ui ──▶ JobQueueEvent ──▶ RemoveFromQueue / ChooseForQueue
  └── the page: settings_ui ──▶ SettingsEvent ──▶ Action::Settings
apply(&mut self, actions)
  └── queue_editing, or actions::settings (save, choosers, downloads, checks)
```

`frame_ui` borrows the state immutably and only collects actions; `apply` and `poll` are the only
places the state changes, between frames. A chooser opens through `core::portal` on its own
thread; its answer goes into the queue or the settings draft.

## Boundaries

- Depends on: `crate::job_queue` and `crate::settings` (events, models, services and ui);
  `crate::core` (`background`, `portal`, `ui`); `inference::model_store` for the runtime folder;
  `eframe`, `anyhow` and `tracing`.
- Used by: `crate::cli`, which calls `launch` for the `gui` subcommand and for no subcommand.
- Rules:
  - nothing changes state while a frame is drawn: every change is an `Action` applied after the
    frame (`actions_change_the_queue_only_when_applied` in `tests/rendering.rs`), and an edit is
    written only by Save (`an_edit_is_saved_only_by_save`);
  - an empty queue tells the user how to add videos, queued videos show by file name, and the
    settings page shows the form, the models and the checks
    (`an_empty_queue_says_how_to_add_videos`, `queued_videos_show_by_file_name`,
    `the_settings_page_shows_the_form_models_and_checks`);
  - the tests never read or write the owner's files (`Environment::scratch`);
  - no feature imports this module
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the two-pane window and the borrowed-view
  pattern it follows.

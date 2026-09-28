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
├── tests/            headless tests of the frame and of applying actions; the snapshot scenes
└── window.rs         one frame: the colour scheme, page tabs, the queue on the left, the page on the right
```

## How it works

`launch` opens a native window titled "TBD Subtitles" (application id `tbd-subtitles`), 1200 by
760 and at least 760 by 480, with drag and drop on and the glow renderer, and returns when it
closes. It runs under X11 (XWayland on the owner's KDE Wayland desktop, forced through winit's
`with_x11`), because only there can files be dropped onto the window and Settings be placed over
it. Before the first frame it installs the theme from `core::ui::theme` (Adwaita Sans, the icon
font, the mockup's palettes) and starts following the desktop's colour scheme through
`core::color_scheme`, waiting up to 250 ms for its first answer so the first frame already has the
desktop's colours. Its `Environment` names the owner's settings file, the kept queue, the GPU
lock and the runtime folder, holds the job runner (the pipeline's `run_job`) and wakes the window
from any thread (`request_repaint`); the tests build one over a scratch folder with a stand-in
runner, and start no portal thread. `TbdSubtitlesApp` holds the page shown (Jobs or Settings), the
queue loaded from `queue.json`, two job runners with the cancel token of the job each runs (one
for full runs, one for the review runs that re-time the owner's corrections), the step rates for
the time left, the settings page, the desktop's colour scheme, the selected finished job's report,
its line review while open, the clip playing in it, and `Pending`, the receiving end of every
other thread it started. The videos passed to `launch`
enter the queue as the first `Action`; the machine checks and the work folder's measure start at
once. While a job runs the window redraws every second; a playing clip wakes it at each frame.

Each frame runs in three steps:

```text
poll(&mut self)        the threads' answers: chooser paths, downloads, checks, sizes, job events,
                       the colour scheme
theme::follow          light or dark, as the desktop reported
frame_ui(&self)
  ├── dropped files ──▶ Action::QueueVideos
  ├── page tabs ──▶ Action::ShowPage
  ├── feature_views::queue_ui ──▶ JobQueueEvent ──▶ Action::Queue
  └── the page: jobs_ui (the review, or progress then the report) or settings_ui
                  ──▶ Queue / Report / Review / Settings
apply(&mut self, actions)
  └── actions::queue (edits, Start, Pause, Cancel, the next job of each lane), actions::review
      (open, edit, save and queue a review run, play), actions::report or actions::settings
```

`frame_ui` borrows the state immutably and only collects actions; `apply` and `poll` are the only
places the state changes, between frames. A chooser opens through `core::portal` on its own
thread; its answer goes into the queue or the settings draft.

## Boundaries

- Depends on: `crate::job_queue`, `crate::job_report`, `crate::line_review` and
  `crate::settings` (events, models, services and ui); `media_io::preview` for the clip;
  `crate::core` (`background`, `color_scheme`, `portal`, `ui`); `inference::model_store` for the
  runtime folder; `eframe`, `winit` (the X11 event loop), `anyhow` and `tracing`; in the snapshot
  test only, `egui_kittest` and `image`.
- Used by: `crate::cli`, which calls `launch` for the `gui` subcommand and for no subcommand.
- Rules:
  - nothing changes state while a frame is drawn: every change is an `Action` applied after the
    frame (`actions_change_the_queue_only_when_applied` in `tests/rendering.rs`), and an edit is
    written only by Save (`an_edit_is_saved_only_by_save`);
  - an empty queue tells the user how to add videos, queued videos show by file name and wait
    for Start, and the settings page shows the form, the models and the checks
    (`an_empty_queue_says_how_to_add_videos`,
    `queued_videos_show_by_file_name_and_wait_for_start`,
    `the_settings_page_shows_the_form_models_and_checks`);
  - jobs run one after another and a cancelled job can be retried
    (`started_jobs_run_one_after_another_and_the_queue_is_kept`,
    `a_cancelled_job_ends_cancelled_and_can_be_retried`), a finished job shows its report
    (`a_finished_job_shows_its_report`), and a saved correction queues a review run that starts
    at once (`a_saved_correction_queues_a_review_run_that_runs_at_once`);
  - the window draws in the desktop's scheme and follows its changes
    (`the_window_draws_in_the_desktops_scheme`, `a_change_of_the_desktops_scheme_is_followed`);
    the rendering harness installs the theme;
  - the tests never write the owner's files (`Environment::scratch`), and read them only in the
    ignored snapshot test `window_snapshots` (`tests/window_snapshots.rs`), which copies the JSON
    files of the Dressrosa 11 and 15–17 work folders into a scratch folder and writes PNGs of the
    queue, the Dressrosa 15 report, its review and the settings page, light and dark, at 1280 by
    800, to `$TBD_SNAPSHOTS`:
    `TBD_SNAPSHOTS=<folder> cargo test -p tbd_subtitles -- --ignored window_snapshots`, on the
    host, since it renders with wgpu;
  - no feature imports this module
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the two-pane window and the borrowed-view
  pattern it follows.

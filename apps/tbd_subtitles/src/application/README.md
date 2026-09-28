# Application window

The eframe desktop window: the application state, one frame of drawing, the actions that change
the state, and the threads it waits on. It composes the feature modules and is the only module
that draws them.

## Contents

```text
apps/tbd_subtitles/src/application/
├── actions/            applying each feature's actions and folding its threads' answers in
├── background.rs       `Pending`: the threads the window waits on, and the chooser's answers
├── environment.rs      `Environment`: the settings file, the runtime folder, the wake; scratch in tests
├── events.rs           `Action`, built from the features' events
├── feature_views.rs    lends each feature its borrowed view and turns its events into actions
├── mod.rs              `TbdSubtitlesApp`, `apply`, and `launch`, which opens the window
├── settings_window.rs  the Settings window, a second native window centred over the main one
├── shortcuts.rs        Ctrl+O, Ctrl+Shift+O, Ctrl+, , Delete and the arrows
├── tests/              headless tests of the frame and of applying actions; the snapshot scenes
└── window.rs           one frame: toolbar, sidebar, the selected job, drop overlay, toasts, Settings
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
runner, and start no portal thread. `TbdSubtitlesApp` holds the queue loaded from `queue.json`,
two job runners with the cancel token of the job each runs (one for full runs, one for the review
runs that re-time the owner's corrections), the step rates for the time left, the settings page,
whether the Settings window is open, the toasts, the row removed last (for Undo), the desktop's
colour scheme, the selected finished job's report, its line review while open, the clip playing in
it, and `Pending`, the receiving end of every other thread it started. The videos passed to `launch`
enter the queue as the first `Action`; the machine checks and the work folder's measure start at
once. While a job runs the window redraws every second; a playing clip wakes it at each frame.

Each frame runs in three steps:

```text
poll(&mut self)        the threads' answers: chooser paths, downloads, checks, sizes, job events,
                       the colour scheme; toasts whose time is up go
theme::follow          light or dark, as the desktop reported
frame_ui(&self)
  ├── dropped files ──▶ Action::QueueVideos
  ├── shortcuts ──▶ Queue(AddVideos / AddFolder / Remove / Select), ShowSettings
  ├── toolbar (52 px) ──▶ JobQueueEvent ──▶ Action::Queue; the gear ──▶ Action::ShowSettings
  ├── sidebar (272 px) ──▶ JobQueueEvent ──▶ Action::Queue
  ├── the selected job: jobs_ui (the empty card, the review, or progress then the report)
  │               ──▶ Queue / Report / Review
  ├── the drop overlay while files hover; the toasts ──▶ Action::ToastButton
  └── settings_window (while open) ──▶ Settings / ShowSettings(false)
apply(&mut self, actions)
  └── actions::queue (edits, Undo, Try Again, Run Again, Start, Pause, Cancel, toasts),
      actions::runner (the next job of each lane, its options, how it ended), actions::review
      (open, edit, save and queue a review run, play), actions::report or actions::settings
```

`frame_ui` borrows the state immutably and only collects actions; `apply` and `poll` are the only
places the state changes, between frames. A chooser opens through `core::portal` on its own
thread; its answer goes into the queue or the settings draft, and a chooser that fails says so in
a red toast. Toasts are added in `apply` (a removed row's "Removed … Its files stay on disk." with
Undo for 6 s, the pause, Try Again and Run Again, a copied path) and drawn at the bottom centre;
their button's action is applied as it was given.

The shortcuts are read before anything is drawn: Ctrl+Shift+O (matched first) adds a folder,
Ctrl+O videos, Ctrl+, opens Settings; Delete removes the selected row and ↑ and ↓ move through the
rows of the open sections, except while a text box has focus or a menu is open. Settings opens in
a second native window through `show_viewport_immediate`, 660 by 600, centred over the main window
when it opens (which X11 allows), and closes when the owner closes it; asking for it while it is
open brings it to the front. Where the backend has one window only, as in the headless tests, egui
draws it as a window inside the main one.

## Boundaries

- Depends on: `crate::job_queue`, `crate::job_report`, `crate::line_review` and
  `crate::settings` (events, models, services and ui); `media_io::preview` for the clip;
  `crate::core` (`background`, `color_scheme`, `portal`, `steps`, `toast`, `ui`); `inference::model_store` for the
  runtime folder; `eframe`, `winit` (the X11 event loop), `anyhow` and `tracing`; in the snapshot
  test only, `egui_kittest` and `image`.
- Used by: `crate::cli`, which calls `launch` for the `gui` subcommand and for no subcommand.
- Rules:
  - nothing changes state while a frame is drawn: every change is an `Action` applied after the
    frame (`actions_change_the_queue_only_when_applied` in `tests/rendering.rs`), and an edit is
    written only by Save (`an_edit_is_saved_only_by_save`);
  - an empty queue tells the user how to add videos, queued videos show by name and wait for
    Start, and the Settings window shows the form and the models
    (`an_empty_queue_says_how_to_add_videos`, `queued_videos_show_by_name_and_wait_for_start`,
    `the_settings_window_shows_the_form_and_the_models`);
  - a running queue shows NOW, UP NEXT and DONE with Pause After This Video, a waiting correction
    run does not enable Start, Delete removes the selected row with an Undo toast that puts it
    back (only the row removed last, and not while its video is in the list again), a job tried
    again while the full lane runs waits for Start Queue, the keys add videos or a folder, open
    Settings and move through the rows, and Run Again with the settings a job ran with runs
    nothing (`a_running_queue_shows_its_sections_and_pause_after_this_video`,
    `a_waiting_correction_run_does_not_enable_start`,
    `delete_removes_the_selected_row_and_undo_puts_it_back`,
    `only_the_row_removed_last_comes_back`, `a_video_already_in_the_list_is_not_put_back`,
    `a_job_tried_again_while_the_full_lane_runs_waits_for_start_queue`,
    `keys_add_videos_or_a_folder_open_settings_and_move_through_the_rows`,
    `run_again_with_the_settings_it_ran_with_runs_nothing` in `tests/rendering_queue.rs`);
  - jobs run one after another and the newest finished comes first, a cancelled job keeps its
    finished steps and Try Again starts it at once without turning the queue on, a failed job
    records its step
    (`started_jobs_run_one_after_another_and_the_queue_is_kept`,
    `a_cancelled_job_keeps_its_finished_steps_and_can_be_retried`,
    `a_failed_job_records_its_step_and_the_steps_it_kept`), a finished job shows its report
    (`a_finished_job_shows_its_report`), a saved correction queues a review run that starts at
    once and shows on its video's one row
    (`a_saved_correction_queues_a_review_run_that_runs_at_once`), and a full run waits while its
    video's review run runs (`a_full_run_waits_while_its_videos_review_run_runs`);
  - the window draws in the desktop's scheme and follows its changes
    (`the_window_draws_in_the_desktops_scheme`, `a_change_of_the_desktops_scheme_is_followed`);
    the rendering harness installs the theme;
  - the tests never write the owner's files (`Environment::scratch`), and read them only in the
    ignored snapshot test `window_snapshots` (`tests/window_snapshots.rs`), which copies the JSON
    files of the Dressrosa 11 and 15–17 work folders into a scratch folder and writes PNGs of the
    finished queue, the Dressrosa 15 report, its review, the Settings window, the first run, a
    running queue, a waiting row's menu and the Undo toast, light and dark, at 1280 by 800, to
    `$TBD_SNAPSHOTS`:
    `TBD_SNAPSHOTS=<folder> cargo test -p tbd_subtitles -- --ignored window_snapshots`, on the
    host, since it renders with wgpu;
  - no feature imports this module
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the window's toolbar, sidebar and detail pane,
  and the borrowed-view pattern it follows.

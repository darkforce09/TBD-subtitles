# Job queue views

The toolbar across the top of the window, the sidebar of videos on the left with each row's menu,
the empty list's card, the drop overlay, and the selected job's progress on the right, drawn from
a borrowed view; they return events and change nothing.

## Contents

```text
apps/tbd_subtitles/src/job_queue/ui/
├── drop_overlay.rs   the blue wash and card while files are dragged over the window
├── empty_state.rs    the card in the middle while the list is empty: drop here, or the add buttons
├── mod.rs            the module list and the entry points
├── progress_view.rs  the selected job: stage, time left, a row per step, or where it stands
├── row_menu.rs       a row's right-click menu, by the row's state
├── sidebar.rs        the sections Now, Up Next and Done with their headings, or the empty hint
├── sidebar_row.rs    one row: status mark, name, status line, bar, ✕ on hover, drag and drop
└── toolbar.rs        Add Videos…, Add Folder…, the queue button with its reason, the gear
```

## How it works

`toolbar_ui` draws Add Videos… and Add Folder… on the left and, on the right, the reason the queue
button is off with an info mark, the button `queue_editing::queue_control` chooses (the blue Start
Queue or Resume Queue, or Pause After This Video) and the gear, which it reports so the
application opens Settings. `sidebar_ui` draws the rows of `sidebar_rows::rows` under the headings
NOW, UP NEXT and DONE with their counts; a click on a heading folds its section away (kept in
egui's memory), and `row_order` gives the rows the arrow keys move through. With no video it shows
a film strip, "No videos yet" and how to add some.

`sidebar_row_ui` draws a row 48 px high (a running row is taller by its bar): the status mark (a
clock, a progress ring, a check, a warning, a cross, a stop or a spinner for a correction run), the
name and the status line from `status_text`, cut with an ellipsis, and a running job's thin bar; a
selected row is drawn on the accent with white text. A click anywhere selects it, a right click
selects it and opens `row_menu_ui`; while the pointer is on a row that can leave the list, a red
round ✕ takes it out. A waiting full run carries its id as egui's drag payload: over another
waiting row a line shows whether it lands before or after it, and the drop asks to move it there.
`row_menu_ui` lists the commands of the row's state, with the ones that cannot apply disabled: Run
Next, Move Up and Move Down for a waiting row; Cancel for a running one, and Stop Updating
Subtitles for a finished one whose correction run runs; Check Lines, Open in
Player, Show in Folder, Copy Subtitle Path (which puts the path on the clipboard) and Run Again with
Current Settings for a finished one; Try Again for a failed or cancelled one; and Remove from List
(Delete) for any that is not running.

`empty_state_ui` draws the card with a dashed border in the middle of the right side: "Drop videos
here", what the app does, that the models download first while one is missing, and the blue Add
Videos… beside Add Folder…. `drop_overlay_ui` covers the window while files hover over it: "Drop
to add videos" and that a folder adds only its videos without subtitles.
`progress_view_ui` draws the selected job: while it runs, what its stage is doing ("Settling the
words…"), its elapsed time, time left and a row per step by its plain title; a waiting job's place
in line ("2nd in line"); a failed job's stage and step ("Failed at Hear the speech.", "Listen with
Whisper: …") and the finished steps it kept; a cancelled job's kept steps.

## Boundaries

- Depends on: `crate::job_queue::{events, models, services}`; `crate::core::{format, steps, ui}`;
  `eframe`.
- Used by: `crate::application` (`feature_views`, `window`, `shortcuts`).
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a running row offers no ✕ and no Remove,
  and only waiting full runs drag (the headers of `sidebar_row.rs` and `row_menu.rs`); the
  toolbar, sections, rows, toasts and shortcuts render as tested in
  `apps/tbd_subtitles/src/application/tests/rendering_queue.rs`.

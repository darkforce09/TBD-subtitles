# Job queue views

The toolbar across the top of the window, the sidebar of videos on the left with each row's menu,
the empty list's card, the drop overlay, and the selected job's cards, progress and stages on the
right, drawn from a borrowed view; they return events and change nothing.

## Contents

```text
apps/tbd_subtitles/src/job_queue/ui/
├── drop_overlay.rs   the blue wash and card while files are dragged over the window
├── empty_state.rs    the card in the middle while the list is empty: drop here, or the add buttons
├── job_cards.rs      the waiting, failed and cancelled cards with their buttons
├── mod.rs            the module list and the entry points
├── progress_card.rs  a running job's stage, step, thick bar, time left and time so far
├── progress_view.rs  the selected job's cards by its state: the dispatcher
├── row_menu.rs       a row's right-click menu, by the row's state
├── sidebar.rs        the sections Now, Up Next and Done with their headings, or the empty hint
├── sidebar_row.rs    one row: status mark, name, status line, bar, ✕ on hover, drag and drop
├── stage_list.rs     the six stages with their marks and times, "Show all 18 steps", step lines
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
clock, a progress ring, a check, a warning for a finished job that does not pass the quality
check, a cross, a stop or a spinner for a correction run), the name and the status line from
`status_text`, cut with an ellipsis, and a running job's thin bar; a finished row that passes and
has lines left to check ends in the orange count of them (`core::ui::pill::paint_badge`, white on
a selected row); a selected row is drawn on the accent with white text. A click anywhere selects
it, a right click selects it and opens `row_menu_ui`; while the pointer is on a row that can
leave the list, a red round ✕ takes the count's place and takes the row out. A waiting full run carries its id as egui's drag payload: over another
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
`progress_view_ui` draws the selected job's cards under the application's header, in a column
16 px apart, by its state; a finished job's body is the application's. A waiting job's card
(`job_cards`) has a clock, its place ("Next in line", "2nd in line"), when it starts ("It starts
when the current video finishes.", "Press Start Queue to begin.", "It can start once the models
are on disk.") and that it drags in the sidebar, its path on the recessed well, Run Next (off for
the first in line) and the red Remove from List; a waiting correction run says it starts as soon
as the video is free, with Remove from List only. A running job gets `progress_card` over
`stage_list`: the progress card writes what its stage is doing in 17 px semibold ("Settling the
words"), the step at work ("Now: Language model settles the words · step 9 of 18"; between two
steps the last one started, never the shot scan), a 6 px bar of
the share done, and the time left ("about 4 min left", or "Working out the time left…" until the
video's length is known) beside the time so far ("10 min 00 s so far"). The stage list is a card
with "Show all 18 steps" on top (Hide the 18 steps once open, "6 stages" on the right; one per job,
in egui's memory), then a row per stage from `stage_progress`: a check with its time or "already
done", a ring filled to its share with the time "so far", a red cross with "failed", or a grey
empty ring and a grey title for a stage to come. A running or failed stage shows its steps, or
every stage does while the disclosure is open: each step's plain title, then "already done",
"done" in green with its time (when known), a thin bar with its time so far, "running in the
background" for the shot scan, "failed" in red, or "to run" in grey. A failed job's card has a
red cross, "Failed at Hear the speech", the step in plain words ("Listen with Whisper stopped
with an error."), the finished steps kept, what Try Again continues after ("Try Again continues
after any steps already done." when it kept none) and when it starts ("It starts at once.", "It
runs next, when the current video finishes.", "It goes first in line and waits for Start
Queue.", "It waits until the models are on disk."), the raw message wrapped on the recessed well,
the blue Try Again and Show in Folder, and the stage list under it (the steps it ran done with
their times, those still valid "already done") unless it failed before its first step. A
cancelled job's card has a stop mark, the steps kept, when Try Again starts it, Try Again and
Remove from List.

## Boundaries

- Depends on: `crate::job_queue::{events, models, services}`; `crate::core::{format, steps, ui}`;
  `crate::job_report::models::summary::RowSummary` in `sidebar_row.rs`; `eframe`;
  `job_model::StepName`.
- Used by: `crate::application` (`feature_views`, `window`, `shortcuts`).
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a running row offers no ✕ and no Remove,
  and only waiting full runs drag (the headers of `sidebar_row.rs` and `row_menu.rs`); the
  toolbar, sections, rows, toasts and shortcuts render as tested in
  `apps/tbd_subtitles/src/application/tests/rendering_queue.rs`, and the cards, the progress card
  and the stages as tested in `apps/tbd_subtitles/src/application/tests/rendering_detail.rs`
  (`a_running_job_shows_its_stage_its_step_and_its_stages`,
  `a_running_job_works_out_its_time_left_until_its_length_is_known`,
  `a_waiting_job_shows_its_place_and_what_starts_it`,
  `a_job_that_failed_before_its_first_step_shows_no_stages`,
  `a_failed_job_shows_the_steps_it_ran_as_done_with_their_times`) and `tests/rendering.rs`
  (`a_cancelled_job_keeps_its_finished_steps_and_can_be_retried`,
  `a_failed_job_records_its_step_and_the_steps_it_kept`).

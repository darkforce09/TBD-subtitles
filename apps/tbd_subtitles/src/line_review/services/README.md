# Line review services

The review logic, with no rendering code: a job's lines read from its work directory, the owner's
edits saved to `review.json`, and a line's clip played through FFmpeg.

## Contents

```text
apps/tbd_subtitles/src/line_review/services/
├── clip_player.rs     the clip's sound to `pulse` and its frames to the window, on a thread
├── mod.rs             the module list
├── review_editing.rs  open a line, pick a reading, save or take back a correction, next line
├── review_loading.rs  the sheet, re-decodes, adjudication, check and corrections as a session
└── tests/             unit tests for loading and editing
```

## How it works

`review_loading::load` joins, per utterance in sheet order, the engines' readings from
`sheet.json` (`P`, `W`) and the re-decodes (`ALT p`, `ALT w`), the settled text and flags from
`adjudicated.json`, the findings that name it in `qc.json`, and the corrections in `review.json`.
`review_editing::save` records where the text came from (an engine's tag, the settled text, or
typed), drops `UNSURE`, refuses an empty text unless the line is dropped, and writes the whole file
through a part file; taking back the last correction removes the file. `clip_player::play` runs
both FFmpeg processes from a thread that waits for them, paces the frames at 12 per second from the
clip's start, and stops both through one flag.

## Boundaries

- Depends on: `crate::line_review::models`; `crate::core::background::Wake`; `job_model`;
  `media_io::preview`; `child_process`; `pipeline::work_dir::job_id`; `serde` and `serde_json`.
- Used by: `crate::application::actions::review`; `crate::line_review::ui` (`EDITABLE_FLAGS`).
- Rules:
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - every reading of a line is gathered in sheet order
    (`every_reading_of_a_line_is_gathered_in_sheet_order` in `tests/review_loading.rs`);
  - a picked reading is saved as that engine's, a typed text as typed, and an empty text only for
    a dropped line (`a_picked_reading_is_saved_as_that_engines_and_written`,
    `a_typed_text_is_typed_and_an_empty_one_is_refused_unless_dropped` in
    `tests/review_editing.rs`).

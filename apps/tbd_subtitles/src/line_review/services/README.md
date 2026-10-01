# Line review services

The review logic, with no rendering code: a job's lines read from its database, filtered
for the list, the owner's edits saved to the job's line corrections, and a line's clip and still frame decoded
by FFmpeg.

## Contents

```text
apps/tbd_subtitles/src/line_review/services/
├── clip_player.rs     a clip's sound to `pulse` and frames to the window, its place; a line's still frame
├── line_filter.rs     the lines of each list, narrowed to a group and a search; counts, typed times, neighbours
├── mod.rs             the module list
├── review_editing.rs  drafts; save, looks right, take back; the correction run's states; carry over
├── review_loading.rs  the sheet, re-decodes, adjudication, check and corrections as a session
└── tests/             unit tests for loading, filtering, editing and the playhead
```

## How it works

`review_loading::load` joins, per utterance in sheet order, the engines' readings from
the sheet (`outputs/diff_sheet`: `P`, `W`) and the re-decodes (`outputs/redecode_parakeet`,
`outputs/redecode_whisper`: `ALT p`, `ALT w`), the settled text and flags from the
re-adjudication (`outputs/readjudicate`), the groups of the findings that name it in the quality
check (`outputs/qc`) (each with why in the
owner's words: the word both engines heard, the reading speed, the layout rule), and the
line corrections, all in one read of the job's database; a line with a Fix It change the owner has not checked is in the
Changed by Claude group first, with what the app had and Claude's reason.

`line_filter::shown` lists To Check (worth a listen, not settled by the owner), Checked (settled
by the owner) or every line, narrowed to the session's group and search: words of the line as it stands now, its id, or a
typed time (`16:33` is the second from 16:33, `16:33.4` its tenth, `16:3` the ten seconds from
16:30; a time too large to count is none), which matches the lines said then. A line is worth a
listen while the quality check flags it, the owner corrected it, or the owner took it back and
its run has not ended, since a correction run drops the findings of the lines it settles; `worth`
counts them for "All N lines checked", and `counts` gives the three lists' sizes whatever the
group and search, `open_line` the line the editor shows (the open one while listed, else the
first), and `neighbour` the line before or after it.

`review_editing` keeps a draft per line only while it differs from what the line has saved, so
opening another line keeps it. `save` records where the text came from (an engine's tag, the
settled text, or typed), drops `UNSURE`, refuses an empty text unless the line is dropped, and
changes the file through `pipeline::work_dir::update_corrections`, under its lock and as it is
on disk, so a Fix It change written meanwhile is kept; `looks_right` saves the line unchanged:
the language model's reading, or a Fix It change kept as the owner's (`kept_fix_it`), and
`undo_change` saves the language model's reading in place of a Fix It change. Both mark the line Saved and return the next line of the list, which
they open: the one after it when the list still shows it (the line itself when it is the last),
else the one now in its place. `revert` keeps the line it takes back open, so its run's status
shows: when its list (Checked) no longer shows it, the list becomes All, and only a search that
no longer matches its text moves the editor on. Taking back the last correction removes the
file; a line with no correction has nothing to take back (`revert` says so). `run_started`
turns Saved and Failed lines to Updating and `run_ended` Updating ones to Updated or Failed
(`start_runs` and `end_runs` do the same for a closed review's runs; `mark_fixed` marks the
lines a Fix It run changed as Saved); `carry_over` keeps the open
line, the list, the search, the group, the runs and the drafts still worth keeping when the
lines are read again, and `park` and `unpark` keep a closed review's drafts and runs until it
opens again.

`clip_player::play` runs both FFmpeg processes from a thread that waits for them, paces the
frames at 12 per second from one start clock taken before either process spawns, and stops both
through one flag. The sound runs about a second past the clip: `media_io::preview` ends it with a
silence pad so the sound server plays the clip to its end before FFmpeg exits, and the clip
stays playing until then. `position_s` is the clip's start plus the time since its sound's
FFmpeg started, never past its end. `still` decodes one frame at a time on a thread (about
150 ms) and reads what else FFmpeg writes to its end, so FFmpeg exits and is waited for. Every
frame carries a serial, so the view uploads each once.

## Boundaries

- Depends on: `crate::line_review::models`; `crate::core::background::Wake`;
  `crate::job_report::models::finding_group`; `job_model`; `media_io::preview`; `child_process`;
  `pipeline::work_dir::{job_id, read_stored}` (the job's rows).
- Used by: `crate::application` (`actions::review`, `actions::report`, `feature_views`,
  `shortcuts`); `crate::line_review::ui` (`line_filter`, `review_editing::{status, is_dirty,
  EDITABLE_FLAGS}`, `clip_player::PAD_S`).
- Rules:
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - every reading of a line is gathered in sheet order, with its groups
    (`every_reading_of_a_line_is_gathered_in_sheet_order` in `tests/review_loading.rs`);
  - a draft survives switching lines, a save moves on, Looks Right saves the language model's
    reading, the last line of a list that keeps it stays open, a line taken back stays open,
    nothing is taken back from a line without a correction, and the runs move Saved → Updating →
    Updated or Failed
    (`a_draft_survives_switching_lines_and_goes_when_it_matches_again`,
    `a_picked_reading_is_saved_as_that_engines_and_the_list_moves_on`,
    `looks_right_saves_the_language_models_text_unchanged`,
    `reverting_the_last_correction_removes_the_file_and_waits_for_a_run`,
    `a_saved_line_follows_its_correction_run`,
    `saving_the_last_line_of_a_list_that_keeps_it_stays_on_it`,
    `nothing_is_taken_back_from_a_line_without_a_correction`,
    `what_the_owner_did_is_carried_over_to_the_lines_read_again` in `tests/review_editing.rs`);
  - the lists, the search by words, ids and times, and the open line follow `line_filter`
    (`tests/line_filter.rs`); the playhead never passes the clip's end (`tests/clip_player.rs`).

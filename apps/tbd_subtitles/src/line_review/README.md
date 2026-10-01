# Line review

The feature where the owner checks the lines the quality check flagged (Check Lines): a list of
the lines to check, those checked, or every line, narrowed to a group or a search; and for the
line open, why it is worth a listen, its clip (the video's sound or the voices alone, with its
picture and a playhead), what every engine heard and what the language model settled on, the
text and its flags; Save Correction, Looks Right or Take Back, and for a line
[Fix It](/documentation/glossary.md#fix-it) changed, Keep Change or Undo Change. The job's review
step then times the line again and the subtitle file is written again, while a status chip
follows that run.

## Contents

```text
apps/tbd_subtitles/src/line_review/
├── events.rs  `ReviewEvent`: open, edit, save, looks right, undo a change, take back, filter, play
├── mod.rs     the module tree
├── models/    the review session, its lines, drafts, list and runs, and a clip's frames and sound
├── services/  loading a job's lines, filtering them, editing and saving, playing a clip
└── ui/        the list, the editor, the readings and the clip
```

## How it works

```text
Check Lines tab / Check Lines / a group's row ──▶ review_loading::load (sheet, re-decodes,
                                                  adjudicated, qc, corrections, probe)
list ──▶ line_filter (To Check / Checked / All, group, search) ──▶ open line
editor ──▶ ReviewEvent ──▶ review_editing (drafts; save, keep, undo, take back ──▶ corrections)
                      └──▶ clip_player (FFmpeg: sound to pulse, frames to the window; a still)
save ──▶ the queue: a review run of the video ──▶ review, cues, qc, output
run starts / ends ──▶ review_editing::run_started / run_ended ──▶ the status chip
```

A line is worth a listen while the quality check puts it in a group (`job_report`'s
`LineGroup`), the owner corrected it, or the owner took it back and its run has not ended, since
a correction run drops the findings of the lines it settles; To Check lists those the owner has
not settled (a Fix It change waits there, in the Changed by Claude group, until the owner keeps
or undoes it), Checked those the owner settled, and the counts match the header's. Every line starts from its correction, else the language model's text and
flags without `UNSURE`; an edit is a draft of that line, kept while the owner opens other lines
and when the lines are read again, and parked by the application, with the runs of the saved
lines, while Check Lines is closed.
Save Correction writes the draft; Looks Right writes the line unchanged as the language model's
reading, so the review step times it again and its warnings clear; Keep Change makes a Fix It
change the owner's as it is, and Undo Change writes the language model's reading in its place; both move on to the next line
of the list and queue one review run of the video, which runs at once on a review lane of its own
beside a full job and up to three other videos' correction runs (its one model step runs on the
CPU), unless a full run of the same video is running; saving is disabled then. The saved line is Saved until the run starts, Updating subtitles… while
it runs and Subtitles updated (or not) when it ends. A clip plays 0.75 s either side of the
line: one FFmpeg sends the sound to the desktop's sound server through its `pulse` output, a
second decodes 360-line RGBA frames at 12 per second that the window shows in time with it, and
the playhead follows the time since the sound started; Stop kills both. When a line opens, a
third FFmpeg decodes the frame at its start on a thread (about 150 ms), shown before Play.

## Public surface

- `events::ReviewEvent`, `models::{session, clip}`, `services::{review_loading, review_editing,
  line_filter, clip_player}` and `ui::{ReviewView, Playing, review_view_ui}`, for the
  application.

## Boundaries

- Depends on: `job_model::outputs` (the sheet, re-decodes, adjudication, corrections),
  `job_model::report`, `crate::job_report::models::finding_group` (the groups), `media_io::preview`,
  `child_process`, `pipeline::work_dir::{job_id, read_stored}`, `crate::core`; `eframe` in
  `ui/` only.
- Used by: `crate::application` (`actions::review`, `actions::report`, `feature_views`,
  `shortcuts`).
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`, `models/` and
  `services/` never name egui or eframe, and the feature imports neither `application` nor `cli`
  (`module_roots_and_documentation_describe_the_entire_source_tree`,
  `dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a saved correction queues a review run
  that runs at once (`a_saved_correction_queues_a_review_run_that_runs_at_once` in
  `apps/tbd_subtitles/src/application/tests/rendering.rs`); Check Lines is drawn and driven as
  `apps/tbd_subtitles/src/application/tests/rendering_review.rs` checks.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the review behaviour.
- [Pipeline](/documentation/architecture/pipeline.md#7-forced-alignment) — the review step that
  times a correction.
- [Clips play through FFmpeg](/documentation/decisions/desktop_gui.md#2026-09-26--clips-play-through-ffmpeg-not-libmpv)
  — why FFmpeg plays the clip.

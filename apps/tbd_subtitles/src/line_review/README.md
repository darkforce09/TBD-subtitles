# Line review

The feature where the owner fixes the lines the pipeline was unsure about: for each flagged line,
play its clip (the video's sound or the voices alone, with a small picture), see what every
engine heard and what the language model settled on, take one of them or type the text, set its
flags, and save it; the job's review step then times the line again and the subtitle file is
written again.

## Contents

```text
apps/tbd_subtitles/src/line_review/
├── events.rs  `ReviewEvent`: open, pick, edit, flags, save, revert, step, play, stop, close
├── mod.rs     the module tree
├── models/    the review session and its lines, and a clip's frames and sound
├── services/  loading a job's lines, editing and saving corrections, playing a clip
└── ui/        the review view
```

## How it works

```text
report ──Review lines / Review──▶ review_loading::load (sheet, re-decodes, adjudicated, qc,
                                                      review.json, probe)
review view ──▶ ReviewEvent ──▶ review_editing (open, pick, save ──▶ review.json)
                           └──▶ clip_player (FFmpeg: sound to pulse, frames to the window)
save ──▶ the queue: a review run of the video ──▶ review, cues, qc, output
```

A line is flagged when a quality-check finding names it or the language model left it unsure;
"Show every line" opens the others too. Opening a line starts from its correction, else the
language model's text and flags without `UNSURE`. Saving writes the whole corrections file and
queues one review run of the video, which runs at once on its own runner beside a full job (its
one model step runs on the CPU), unless a full run of the same video is running; Save is disabled
then. A clip plays 0.75 s either side of the line: one FFmpeg sends the sound to the desktop's
sound server through its `pulse` output, a second decodes 360-line RGBA frames at 12 per second
that the window shows in time with it; Stop kills both. When the run ends the lines are read
again.

## Public surface

- `events::ReviewEvent`, `models::{session, clip}`, `services::{review_loading, review_editing,
  clip_player}` and `ui::{ReviewView, review_view_ui}`, for the application.

## Boundaries

- Depends on: `job_model::outputs` (the sheet, re-decodes, adjudication, corrections),
  `job_model::report`, `media_io::preview`, `child_process`, `pipeline::work_dir::job_id`,
  `crate::core`, `serde_json`; `eframe` in `ui/` only.
- Used by: `crate::application` (`actions::review`, `feature_views`).
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`, `models/` and
  `services/` never name egui or eframe, and the feature imports neither `application` nor `cli`
  (`module_roots_and_documentation_describe_the_entire_source_tree`,
  `dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a saved correction queues a review run
  that runs at once (`a_saved_correction_queues_a_review_run_that_runs_at_once` in
  `apps/tbd_subtitles/src/application/tests/rendering.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the review behaviour.
- [Pipeline](/documentation/architecture/pipeline.md#7-forced-alignment) — the review step that
  times a correction.
- [Clips play through FFmpeg](/documentation/decisions/desktop_gui.md#2026-09-26--clips-play-through-ffmpeg-not-libmpv)
  — why FFmpeg plays the clip.

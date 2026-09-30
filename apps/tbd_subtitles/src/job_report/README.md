# Job report

The feature that shows a finished job's Overview: its subtitle file and whether it passes the
quality check, each problem in plain words with its fix, its lines worth a listen in the groups
the owner recognises, the check's numbers and each step's time and memory; and the summary its
sidebar row and header show. From it the owner opens the video, the file's folder and
`report.md` through the desktop, opens Check Lines, tries a failed language-model call again, or
has [Fix It](/documentation/glossary.md#fix-it) fix the flagged lines.

## Contents

```text
apps/tbd_subtitles/src/job_report/
├── events.rs  `ReportEvent` and `LinesToCheck`: open, show, copy, Check Lines, Try Again, Fix It, Stop
├── mod.rs     the module tree
├── models/    `JobReport`, `LineGroup`, `Problem`, `Remedy`, `FixResult`, `LineCounts`, `RowSummary`
├── services/  a job's files read into a `JobReport` or `RowSummary`; its counts; Fix It's result, thread
└── ui/        the Overview: the file card with Fix It's result, the localized video, the lines card, Details and Step times
```

## How it works

When the owner selects a finished job, or the selected job ends, the application reads its report
through `services::report_loading::load`: the video's work directory is found as the pipeline
names it (the canonical path's job id under the work folder), then `job.json` gives the steps'
measures, `qc.json` the quality check, `output.json` the subtitle file, `review.json` (when a line
was corrected) the corrections and `fix.json` (when Fix It ran) what Fix It answered.
`services::line_counts` counts the lines worth a listen: each finding about a line falls in one
`LineGroup` (Unsure what was said, Heard word replaced, Word no engine heard, Too fast to read,
Loosely timed, Layout), a line counts once however many findings name it, and a line the owner
corrected is worth a listen and checked, even once a correction run has settled its findings (then
under no group), so "1 of 38 checked" holds across the run; the lines Fix It changed, and those
whose every finding it answered, count as checked by Claude. The findings about the whole job
become `Problem`s, one per pass rule the job breaks (layout, speech with no subtitle, the
aligner's offset, a failed language-model call, reading speed), so there are none exactly when
`QcReport::passes` holds. `services::fix_result` sums up what Fix It did, from its record, the
corrections and the problems now. `report_loading::summary` reads `qc.json`, `review.json` and
`fix.json` for a row's `RowSummary` (its problems, lines worth a listen, lines to check and whether
Claude fixed it), which the application keeps for every finished row: read when the window opens,
after each run of its video and after each correction.

The Overview draws the file card ("Subtitles saved next to the video", the green "Passes the
quality check" or orange "Needs attention" pill, the green Fix It result once Fix It has answered
lines, the problems with their buttons, the blue notes while Fix It or a correction run updates the
file, the path, Open in Player, Show in Folder and Copy Path), the lines card ("38 lines worth a
listen", the green bar of those checked, the blue Check Lines, and a row per group that opens
Check Lines on it), and the closed disclosures Details and Step times. Open asks the desktop portal, so the video opens in the desktop's default player (VLC) and
the app starts no program. Try Again beside a failed language-model call runs the job again from
adjudication (`StepName::Adjudicate`): every model call and every step after them. Check Lines on
a group opens the line review on the lines to check narrowed to that group; Show Nearby Lines
opens it with every line shown, at the line nearest the first stretch of speech with no subtitle.

## Public surface

- `models::{report, finding_group, fixing, problem, summary}`,
  `services::{report_loading, line_counts, fix_it}`, `ui::{OverviewView, overview_ui}` and
  `events::{ReportEvent, LinesToCheck}`, for the application.
- `models::fixing::FIX_STEPS`, for the queue's status line of a video Fix It fixes.
- `models::summary::RowSummary`, for the queue's sidebar rows; `crate::line_review` may import
  `models` too, for the groups of its lines.

## Boundaries

- Depends on: `job_model` (`JobRecord`, `OutputRecord`, `Corrections`, `FixRecord`, `QcReport`,
  `QcCheck`, `TimingSource`), `pipeline::work_dir::{job_id, WorkDir}`, `pipeline::fix_it`,
  `stages::output::subtitle_path`, `stages::fix_it`, `serde_json`, `crate::core`,
  `crate::settings::models::claude_models`; `eframe` in `ui/` only.
- Used by: `crate::application` (`actions::report`, `actions::fix_it`, `feature_views`,
  `detail_view`); `crate::job_queue` (`models::summary` in `models::view`,
  `services::status_text` and `ui::sidebar_row`; `models::fixing` in `services::status_text`).
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`, `models/` and
  `services/` never name egui or eframe, no other feature imports `ui/`, and the feature imports
  neither `application` nor `cli`
  (`module_roots_and_documentation_describe_the_entire_source_tree`,
  `dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a finished job shows its Overview, a
  failed language-model call needs attention and Try Again reruns the calls, and a job finished in
  an earlier window shows its real verdict and count (`a_finished_job_shows_its_report`,
  `a_failed_language_model_call_needs_attention_and_try_again_reruns_the_calls`,
  `a_job_finished_in_an_earlier_window_shows_its_verdict_and_lines_to_check` in
  `apps/tbd_subtitles/src/application/tests/rendering_report.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the Overview the window shows.
- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — the checks behind it.

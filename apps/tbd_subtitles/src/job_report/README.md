# Job report

The feature that shows a finished job: whether it passes the quality check and why not, its counts,
its subtitle file with buttons that open the video, its folder and `report.md` through the desktop,
every finding with its time, and each step's time and memory; from it the owner opens the line
review.

## Contents

```text
apps/tbd_subtitles/src/job_report/
├── events.rs  `ReportEvent`: open a file or folder through the desktop, or review the lines
├── mod.rs     the module tree
├── models/    `JobReport`, the finished job as the view shows it
├── services/  reading a job's `job.json`, `qc.json` and `output.json` into a `JobReport`
└── ui/        the report view
```

## How it works

When the owner selects a finished job, or the selected job ends, the application reads its report
through `services::report_loading::load`: the video's work directory is found as the pipeline
names it (the canonical path's job id under the work folder), then `job.json` gives the steps'
measures, `qc.json` the quality check and `output.json` the subtitle file. The view draws it under
the job's progress on the Jobs page. Open asks the desktop portal, so the video opens in the
desktop's default player (VLC) and the app starts no program. Review lines opens the line review
at the first flagged line, and a finding's Review button at the line it names.

## Public surface

- `models::report::JobReport`, `services::report_loading::load`, `ui::report_view_ui` and
  `events::ReportEvent`, for the application.

## Boundaries

- Depends on: `job_model` (`JobRecord`, `OutputRecord`, `QcReport`), `pipeline::work_dir::job_id`,
  `stages::output::subtitle_path`, `serde_json`, `crate::core`; `eframe` and `egui_extras` in `ui/`
  only.
- Used by: `crate::application` (`actions::report`, `feature_views`).
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`, `models/` and
  `services/` never name egui or eframe, and the feature imports neither `application` nor `cli`
  (`module_roots_and_documentation_describe_the_entire_source_tree`,
  `dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a finished job shows its report
  (`a_finished_job_shows_its_report` in `apps/tbd_subtitles/src/application/tests/rendering.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the report the window shows.
- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — the checks behind it.

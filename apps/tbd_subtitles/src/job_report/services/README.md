# Job report services

Reading a finished job's report or its row's summary from its work directory, and counting its
lines worth a listen and its problems, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/services/
├── line_counts.rs     lines per group, lines checked, the problems, a row's summary, a group's first line
├── mod.rs             the module list
├── report_loading.rs  `load` and `summary`: `job.json`, `qc.json`, `output.json` and `review.json`
└── tests/             unit tests for the counts and problems, a finished job and a missing one
```

## How it works

`report_loading::load` finds the video's work directory as the pipeline names it and reads
`job.json` (the steps' measures), `qc.json`, `output.json` (the subtitle file, else the path the
settings give) and `review.json` (no corrections while it does not exist), then counts the lines
and problems through `line_counts`. `report_loading::summary` reads only `qc.json` and
`review.json`, for a sidebar row. `line_counts::line_counts` counts the distinct lines named by a
finding in some group, once per group they have findings in, together with every line the owner
corrected, which is checked: a correction run settles a corrected line's findings, and the line
stays counted, under no group. A finding about no line (a sound cue, the whole job) is no line to
check. `line_counts::problems`
gives one `Problem` per pass rule the check breaks, in the order the file card lists them:
layout, speech with no subtitle (with the time of its first stretch), the aligner's offset,
failed language-model calls, reading speed. `line_counts::first_line` names a group's earliest line, where Check Lines opens on it.

## Boundaries

- Depends on: `crate::job_report::models`; `job_model`; `pipeline::work_dir::job_id`;
  `stages::output::subtitle_path`; `serde` and `serde_json`.
- Used by: `crate::application::actions::report`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a missing or broken file is an error
  naming it, `output.json` and a missing `review.json` aside
  (`a_job_without_a_check_names_the_missing_file`,
  `a_finished_job_reads_back_its_check_files_and_steps` in `tests/report_loading.rs`); the
  problems are empty exactly when the job passes, for each pass rule, and a line counts once per
  group, and a corrected line stays counted after the correction run
  (`problems_are_empty_exactly_when_the_job_passes_for_each_rule`,
  `lines_are_counted_once_per_group_and_checked_when_corrected`,
  `corrected_lines_stay_counted_after_the_correction_run_settles_them` in
  `tests/line_counts.rs`).

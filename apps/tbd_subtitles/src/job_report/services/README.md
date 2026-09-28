# Job report services

Reading a finished job's report or its row's summary from its work directory, counting its lines
worth a listen and its problems, and running Fix It on a thread, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/services/
├── fix_it.rs          Fix It on a thread of its own: its progress, its outcome and Stop
├── line_counts.rs     lines per group, lines checked, the problems, a row's summary, what Fix It asks
├── mod.rs             the module list
├── report_loading.rs  `load` and `summary`: `job.json`, `qc.json`, `output.json` and `review.json`
└── tests/             unit tests for the counts and problems, the loading, and Fix It's thread
```

## How it works

`report_loading::load` finds the video's work directory as the pipeline names it and reads
`job.json` (the steps' measures), `qc.json`, `output.json` (the subtitle file, else the path the
settings give) and `review.json` (no corrections while it does not exist), then counts the lines
and problems through `line_counts`. `report_loading::summary` reads only `qc.json` and
`review.json`, for a sidebar row. `line_counts::line_counts` counts the distinct lines named by a
finding in some group, once per group they have findings in, together with every line the owner
corrected, which is checked: a correction run settles a corrected line's findings, and the line
stays counted, under no group. A line Fix It changed is counted too, in the Changed by Claude
group, and is not checked until the owner keeps or undoes it. `line_counts::fixable` counts the
findings `stages::fix_it::items::asks_about` would ask about, which shows or hides the Fix It
row. A finding about no line (a sound cue, the whole job) is no line to check.
`line_counts::problems` gives one `Problem` per pass rule the check breaks, in the order the file
card lists them: layout, speech with no subtitle (with the time of its first stretch), the aligner's offset,
failed language-model calls, reading speed.

`fix_it::start` runs Fix It (`pipeline::fix_it::fix_video`, or a stand-in in the tests) on a
thread of its own that lives until the run ends, so the `claude` processes it starts end with it.
The thread sends each `FixProgress` and then the outcome, waking the window each time;
`Fixing::poll` keeps the latest progress and hands over the outcome once, and `Fixing::stop`
sets the run's `CancelToken`, which kills the running calls.

## Boundaries

- Depends on: `crate::job_report::models`; `job_model`; `pipeline::work_dir::job_id` and
  `pipeline::fix_it`; `stages::output::subtitle_path` and `stages::fix_it::items`;
  `crate::core::background::Wake`; `serde` and `serde_json`.
- Used by: `crate::application::actions` (`report` and `fix_it`) and `crate::application`'s
  environment, which holds the Fix It runner.
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

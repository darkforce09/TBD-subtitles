# Job report services

Reading a finished job's report from its work directory, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/services/
├── mod.rs             the module list
├── report_loading.rs  `load`: `job.json`, `qc.json` and `output.json` into a `JobReport`
└── tests/             unit tests for a finished job and a missing one
```

## Boundaries

- Depends on: `crate::job_report::models`; `job_model`; `pipeline::work_dir::job_id`;
  `stages::output::subtitle_path`; `serde` and `serde_json`.
- Used by: `crate::application::actions::report`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a missing or broken file is an error
  naming it (`a_job_without_a_check_names_the_missing_file` in `tests/report_loading.rs`).

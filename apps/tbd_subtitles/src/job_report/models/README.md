# Job report models

The data the report view draws, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/models/
├── mod.rs     the module list
└── report.rs  `JobReport`: the video, the work directory, the files, the check and the steps
```

## Boundaries

- Depends on: `job_model` (`StepName`, `StepMeasure`, `QcReport`).
- Used by: `crate::job_report::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); the total step time leaves out the shot
  scan, which runs alongside (`JobReport::total_s`).

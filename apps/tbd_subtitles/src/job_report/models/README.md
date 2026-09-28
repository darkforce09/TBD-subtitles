# Job report models

The data the Overview and the sidebar's finished rows draw, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/models/
├── finding_group.rs  `LineGroup`: the six groups of line findings, each with its title, chip and explanation
├── mod.rs            the module list
├── problem.rs        `Problem` and `Remedy`: a broken pass rule in plain words, its fix and its button
├── report.rs         `JobReport`: the video, the files, the check, the corrections, problems, lines, steps
├── summary.rs        `LineCounts` and `RowSummary`: lines worth a listen, checked, and a row's verdict
└── tests/            unit tests for the groups of the checks, and the problems' words and buttons
```

## How it works

`LineGroup::of` puts each check about a line in one group: `unsure` in Unsure what was said,
`removed_locked` (an agreed word dropped) in Heard word replaced, `novel` in Word no engine heard,
`too_fast` in Too fast to read, `weak_timing` in Loosely timed, and the eight layout checks in
Layout; the checks about the whole job (heard speech with no cue, the aligner's offset, a failed
language-model call) have no group. Each group has its title, the short name of its chip on a row
of Check Lines ("Word replaced", "Too fast"), and a plain explanation, as the mockup words them. A `Problem` is one pass rule a job breaks, titled in the owner's words ("1
language-model call failed", "Only 91.2 % of subtitles are easy to read"), with its fix and, for
speech with no subtitle, a failed call and reading speed, a `Remedy`: Show Nearby Lines (at the
first stretch of speech with no subtitle, which the problem carries), Try Again or Show Lines.
`LineCounts` holds the distinct lines worth a listen (the flagged ones and those the owner
corrected), those the owner checked (corrected) and the lines of each group; `RowSummary` is what a finished row and the header say: its
problems, its lines worth a listen and those still to check. `JobReport::summary` gives a loaded
report's.

## Boundaries

- Depends on: `job_model` (`StepName`, `StepMeasure`, `Corrections`, `QcReport`, `QcCheck`).
- Used by: `crate::job_report::{services, ui}`, `crate::application`, and
  `crate::job_queue` for `summary::RowSummary`; `crate::line_review` for `finding_group::LineGroup`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); every check about a line has one group
  and the job-wide checks none (`every_check_about_a_line_has_a_group_and_the_job_wide_ones_none`
  in `tests/finding_group.rs`); the total step time leaves out the shot scan, which runs alongside
  (`JobReport::total_s`).

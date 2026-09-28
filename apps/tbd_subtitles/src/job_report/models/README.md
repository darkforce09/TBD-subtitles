# Job report models

The data the Overview and the sidebar's finished rows draw, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/models/
├── finding_group.rs  `LineGroup`: the groups of line findings, Fix It's first, each with its words
├── fix_result.rs     `FixResult`: what Fix It changed, left right, cleared and left, with examples
├── fixing.rs         `FixView`: Fix It hidden, ready, off with why, running or updating; its steps
├── mod.rs            the module list
├── problem.rs        `Problem` and `Remedy`: a broken pass rule in plain words, its fix and its button
├── report.rs         `JobReport`: the video, the files, the check, corrections, problems, lines, Fix It
├── summary.rs        `LineCounts` and `RowSummary`: lines worth a listen, checked, and a row's verdict
└── tests/            unit tests for the groups, the problems' words and buttons, Fix It's steps and words
```

## How it works

`LineGroup::ChangedByFixIt` ("Changed by Claude", chip "Claude") comes first and holds the lines
Fix It changed that the owner has not kept or undone; it comes from the corrections, never from a
check. `LineGroup::of` puts each check about a line in one group: `unsure` in Unsure what was said,
`removed_locked` (an agreed word dropped) in Heard word replaced, `novel` in Word no engine heard,
`too_fast` in Too fast to read, `weak_timing` in Loosely timed, and the eight layout checks in
Layout; the checks about the whole job (heard speech with no cue, the aligner's offset, a failed
language-model call) have no group. Each group has its title, the short name of its chip on a row
of Check Lines ("Word replaced", "Too fast"), and a plain explanation, as the mockup words them. A `Problem` is one pass rule a job breaks, titled in the owner's words ("1
language-model call failed", "Only 91.2 % of subtitles are easy to read"), with its fix and, for
speech with no subtitle, a failed call and reading speed, a `Remedy`: Show Nearby Lines (at the
first stretch of speech with no subtitle, which the problem carries), Try Again or Show Lines;
`cleared_title` says it in the past tense once Fix It cleared it ("1 subtitle broke a layout
rule", "Only 91.2 % of subtitles were easy to read").
`FixView` is Fix It on the Overview: hidden when it has nothing to ask about, ready with the
model's name ("Claude Opus"), off with why, running with its stage, its calls done and whether
Stop was pressed, or updating while the correction run that puts its changes into the subtitles
waits or runs. The window counts a run in four steps (`FIX_STEPS`): its three passes, then that
correction run (`UPDATING_STEP`); `stage_words` says the stage with its step ("checking each
change (3 of 4)") and `updating_words` the last ("updating the subtitles (4 of 4)").
`FixResult` is what the latest Fix It runs did to a job: the model, the lines changed, the lines
already right, turned down or left to the owner's own correction, the changes the owner kept or
undid, the problems cleared and left, the first two changes' words and how many more changed;
`change_words` says a change as the owner reads it at a glance (“Heaven dish” → “Cavendish”,
Added “Uh,”, Removed “So”).
`JobReport::fixable` counts the findings Fix It would ask about, and `JobReport::fix_result` holds
the result while a Fix It record of the job as it stands answered a line. `LineCounts` holds the
distinct lines worth a listen (the flagged ones, those the owner corrected and those Fix It
changed), those the owner checked (corrected or kept), those Claude checked (its changes the owner
has not kept or undone, and the flagged lines whose every finding it answered) and the lines of
each group; the lines to check are the rest. `RowSummary` is what a finished row and the header
say: its problems, its lines worth a listen, those still to check, and whether Claude fixed the
job. `JobReport::summary` gives a loaded report's.

## Boundaries

- Depends on: `job_model` (`StepName`, `StepMeasure`, `Corrections`, `QcReport`, `QcCheck`);
  `pipeline::fix_it::FixStage` for the stage Fix It is in.
- Used by: `crate::job_report::{services, ui}`, `crate::application`, and
  `crate::job_queue` for `summary::RowSummary`; `crate::line_review` for `finding_group::LineGroup`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); every check about a line has one group
  and the job-wide checks none (`every_check_about_a_line_has_a_group_and_the_job_wide_ones_none`
  in `tests/finding_group.rs`); the total step time leaves out the shot scan, which runs alongside
  (`JobReport::total_s`).

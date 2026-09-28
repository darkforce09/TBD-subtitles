# Job report services

Reading a finished job's report or its row's summary from its work directory, counting its lines
worth a listen and its problems, and running Fix It on a thread, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_report/services/
├── fix_it.rs          Fix It, one thread per run: its progress, its wait for a call, its outcome, Stop
├── fix_result.rs      `fix_result`: what Fix It did, from `fix.json`, the corrections and the problems
├── line_counts.rs     lines per group, lines checked, the problems, a row's summary, what Fix It asks
├── mod.rs             the module list
├── report_loading.rs  `load` and `summary`: `job.json`, `qc.json`, `output.json`, `review.json`, `fix.json`
└── tests/             unit tests for the counts and problems, Fix It's result, the loading, its thread
```

## How it works

`report_loading::load` finds the video's work directory as the pipeline names it and reads
`job.json` (the steps' measures), `qc.json`, `output.json` (the subtitle file, else the path the
settings give), `review.json` (no corrections while it does not exist) and `fix.json` (Fix It's
record, none while it does not exist), then counts the lines and problems through `line_counts`
and what Fix It did through `fix_result`. A Fix It record counts only while it belongs to the
job's re-adjudication as it stands (`FixRecord::is_current`, against `job.json`); a record of an
earlier one counts for nothing. `report_loading::summary` reads `qc.json`, `review.json` and
`fix.json` (with `job.json` when there is one), for a sidebar row. `line_counts::line_counts`
counts the distinct lines named by a finding in some group, once per group they have findings in,
together with every line the owner corrected, which is checked: a correction run settles a
corrected line's findings, and the line stays counted, under no group. A line Fix It changed is
counted too, in the Changed by Claude group, and checked by Claude until the owner keeps or undoes
it; so is a flagged line the owner has not checked whose every finding the Fix It record answered
(`stages::fix_it::items::Answered::covers`). `line_counts::fixable` counts the findings
`stages::fix_it::items::asks_about` would ask about, the answered ones left out, which shows or
hides the Fix It row. A finding about no line (a sound cue, the whole job) is no line to check.
`line_counts::problems` gives one `Problem` per pass rule the check breaks, in the order the file
card lists them: layout, speech with no subtitle (with the time of its first stretch), the
aligner's offset, failed language-model calls, reading speed; `line_counts::problems_of` does the
same from the counts per check, the first speech with no subtitle, the share easy to read and the
cues, as a Fix It record keeps them from before its first run (`FixBefore`).
`fix_result::fix_result` sums up the record: the lines whose change reached the corrections and
differs from before, the answered lines that did not change, those the judge turned down, those
that kept the owner's own correction, the changes the owner kept (`KeptFixIt`) or undid (a
correction of the owner's own), the problems before the first run of which no problem of the same
kind is left (none without a record of them) and the problems left, and the words of the first
two changes in id order (`stages::fix_it::changed_words`) with how many more changed.

`fix_it::start` runs Fix It (`pipeline::fix_it::fix_video`, or a stand-in in the tests) on a
thread of its own that lives until the run ends, so the `claude` processes it starts end with it;
every run has its own thread, so many go at once, their calls sharing the gate whose seat
`FixOptions::calls` carries. `Fixing::waiting` says whether the run has a call waiting for a free
slot there and none holding one (`CallSeat::waiting_only`), which the Overview's note shows.
The thread sends each `FixProgress` and then the outcome, waking the window each time;
`Fixing::poll` keeps the latest progress and hands over the outcome once, and `Fixing::stop`
sets the run's `CancelToken`, which kills the running calls. The thread logs the start, each
new pass and stage at info with its count of calls, each call at debug, and the end, under the
`fix_it` target, in spans naming the video and the step `fix_it`, which the model calls made on
the run's threads inherit.

## Boundaries

- Depends on: `crate::job_report::models`; `job_model`; `pipeline::work_dir::{job_id, WorkDir}`
  and `pipeline::fix_it`; `inference::llm::call_gate::CallSeat`;
  `stages::output::subtitle_path`, `stages::fix_it::items` and `stages::fix_it::changed_words`;
  `crate::settings::models::claude_models` for the model's name; `crate::core::background::Wake`;
  `serde` and `serde_json`.
- Used by: `crate::application::actions` (`report` and `fix_it`) and `crate::application`'s
  environment, which holds the Fix It runner.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a missing or broken file is an error
  naming it, `output.json` and a missing `review.json` or `fix.json` aside
  (`a_job_without_a_check_names_the_missing_file`,
  `a_finished_job_reads_back_its_check_files_and_steps` in `tests/report_loading.rs`); only a
  current Fix It record counts
  (`a_current_fix_record_checks_the_lines_it_answered_and_a_stale_one_counts_for_nothing`); the
  problems are empty exactly when the job passes, for each pass rule, and a line counts once per
  group, and a corrected line stays counted after the correction run, and Claude checks its
  changes and the lines whose every finding it answered
  (`problems_are_empty_exactly_when_the_job_passes_for_each_rule`,
  `lines_are_counted_once_per_group_and_checked_when_corrected`,
  `corrected_lines_stay_counted_after_the_correction_run_settles_them`,
  `claude_checks_its_changes_and_the_lines_whose_every_finding_it_answered` in
  `tests/line_counts.rs`); a problem is cleared only when none of its kind is left
  (`a_problem_is_cleared_when_none_of_its_kind_is_left` in `tests/fix_result.rs`).

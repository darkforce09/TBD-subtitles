# Fix It actions

What the window does for Fix It: what the Overview and the sidebar show about each run, starting
runs on one video, on every finished video with lines to fix or after each full run, Stop, and a
run's end and finish once its changes are in the subtitles.

## Contents

```text
apps/tbd_subtitles/src/application/actions/fix_it/
├── finishing.rs  a run's end: the correction run of its changes, its finish toast, See Changes, notify
├── mod.rs        `FixFollowup`, the Overview's and sidebar's Fix It, why it cannot start, `poll_fix`
└── starting.rs   start on one video, Fix All's candidates and Fix All, Fix It after each job, Stop
```

## How it works

Every run under way sits in `Pending::fixes`, by the job it fixes, at most one per video, each on
a thread of its own (`job_report::services::fix_it`). `starting.rs` starts one with the Fix It
model and processes saved now and a seat at `claude_gate`, the one gate every run's `claude` calls
share, capped at the saved "Claude calls at once"; the gate lets calls through in the order their
runs started. A run starts only where `fix_refusal` in `mod.rs` has nothing against it: the video
is listed, Fix It is not already fixing it, and no run of the video runs and no correction run of
it waits. `fix_candidates` lists the rows of Done whose job finished, whose summary has findings
Fix It would ask about (`RowSummary::fixable`) and against which nothing stands, the newest row of
each video only, oldest finished first; Fix All starts each and says on how many videos, and
`fix_after_run` starts each full run that just finished well that is a candidate, while Fix It
after each job is on. Stop stops the run of the job's video only.

`mod.rs` gives the Overview its `FixView` (running with `waiting` while every call of the run
waits for a free slot, `Fixing::waiting`) and the sidebar each video's step of four, and
`poll_fix` folds in what every run sent before each frame, handing each run that ended to
`finishing.rs`. There a run with changes marks the lines it changed, queues one correction run of
the video and waits in `fix_followups`; once a correction run of the video ended well, or at once
when nothing changed, the run finishes: a green toast with See Changes, the job marked just fixed,
and a desktop notification while the window is away. A stopped or failed run says so in a toast
naming its video; a run refused because another process owns the job says the video is busy and
names that process, which is no failure.

## Boundaries

- Depends on: `crate::application` (`TbdSubtitlesApp`, `Action`, `Pending`, the gate and the
  follow-ups); `crate::job_report` (`services::fix_it`, `models::fixing`, `models::report`,
  events, `models::finding_group`); `crate::job_queue` (`models::queue`, `models::sidebar`,
  `services::sidebar_rows`, `services::queue_editing`, events); `crate::line_review::services::
  review_editing`; `crate::settings` (`models::claude_models`, `services::job_settings`);
  `crate::core` (`format::plural`, `toast`); `pipeline::fix_it` and `pipeline::CancelToken`.
- Used by: `crate::application` (`apply` for `Action::FixIt`, `StopFix` and `SeeFixChanges`,
  `poll` for `poll_fix`, `feature_views` for the view, the steps and the Fix All count) and the
  sibling actions `queue` (Fix All, the refusals), `report` (Fix It and Stop on the selected job)
  and `runner` (the lane rules, `fix_run_ended`, `fix_after_run`).
- Rules: many runs go at once, one per video, and Stop ends only its own; Fix All and Fix It after
  each job start on exactly the finished videos with lines to fix; a run whose every call waits
  says so; the cap follows its setting at once (`two_videos_fix_at_once`,
  `stop_ends_only_its_own_video`,
  `fix_all_starts_every_finished_video_with_lines_to_fix_and_hides_once_all_are_fixing`,
  `fix_after_each_job_starts_when_a_full_run_finishes`, `fix_after_each_job_off_starts_nothing`,
  `a_video_waiting_for_a_free_call_says_so`, `claude_calls_at_once_applies_at_once` in
  `apps/tbd_subtitles/src/application/tests/rendering_fix_many.rs`); a run holds the runs of its
  video, queues one correction run of its changes and finishes once that run ended, or at once
  with nothing changed, and a failed correction run finishes nothing
  (`fix_it_runs_on_the_video_and_queues_the_correction_run_that_times_its_changes`,
  `fix_it_finishes_once_its_changes_are_in_the_subtitles`,
  `fix_it_with_nothing_to_change_finishes_at_once_and_leaves_the_desktop_alone_in_front`,
  `a_failed_correction_run_finishes_nothing` in
  `apps/tbd_subtitles/src/application/tests/rendering_fix_it.rs`).

## Related documentation

- [Fix It](/documentation/features/fix_it.md) — what Fix It does, its passes, and running it on
  many videos at once.

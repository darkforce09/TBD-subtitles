# Job queue models

The data the queue panel and the progress view draw, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_queue/models/
├── mod.rs       the module list
├── progress.rs  `JobProgress` with a `StepRow` and `StepState` per step, and the step `Rates`
├── queue.rs     `Queue`, `QueueItem`, `JobId`, `JobKind`, `JobState`, `Failure`, `JobResult`, `Move`
├── tests/       unit tests for the progress's kept, current and failed steps
└── view.rs      `JobQueueView`, the queue, the rates and the clock lent for one frame
```

## How it works

A `QueueItem` is one video's job: its id, its kind (a full run, or a review run after the owner's
corrections), its state (waiting, running with its progress, finished with its result, finished
in an earlier window, failed with a `Failure`, cancelled with the finished steps it kept), whether
it keeps its own settings (once it has started), the steps its next run does again, and, for a
review run, how many corrections it carries. A `Failure` names the step that failed (none when
the job failed before its first step), the message and the finished steps kept. A `JobProgress`
holds the work directory, the video's length and a row per step: whether this run does it, and
pending, still valid, running (since when, how far, its last line), done (its time) or failed;
it answers how many steps are kept (done, or valid from an earlier run), which step runs now
(beside the shot scan, the later one) and which failed. `Rates` are each step's seconds per
second of video.

## Boundaries

- Depends on: `job_model::StepName`; `std`.
- Used by: `crate::job_queue::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a failed step is never counted as kept
  (`a_failed_step_is_named_and_not_kept` in `tests/progress.rs`).

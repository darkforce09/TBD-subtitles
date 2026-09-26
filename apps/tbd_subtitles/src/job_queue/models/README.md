# Job queue models

The data the queue panel and the progress view draw, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/job_queue/models/
├── mod.rs       the module list
├── progress.rs  `JobProgress` with a `StepRow` and `StepState` per step, and the step `Rates`
├── queue.rs     `Queue`, `QueueItem`, `JobId`, `JobKind`, `JobState`, `JobResult` and `Move`
└── view.rs      `JobQueueView`, the queue, the rates and the clock lent for one frame
```

## How it works

A `QueueItem` is one video's job: its id, its kind (a full run, or a review run after the owner's
corrections), its state (waiting, running with its progress, finished with its result, finished
in an earlier window, failed with the reason, cancelled) and whether it ran in this window. A
`JobProgress` holds the work directory, the video's length and a row per step: whether this run
does it, and pending, still valid, running (since when, how far, its last line), done (its time)
or failed. `Rates` are each step's seconds per second of video.

## Boundaries

- Depends on: `job_model::StepName`; `std`.
- Used by: `crate::job_queue::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

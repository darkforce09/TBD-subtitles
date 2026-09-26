# Line review models

The data the review view draws, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/line_review/models/
├── clip.rs     `Frame` (a numbered RGBA preview frame) and `Sound` (the mix or the voices)
├── mod.rs      the module list
└── session.rs  `ReviewSession`, `ReviewLine`, `Hypothesis` and `Draft`
```

## How it works

A `ReviewSession` holds one job's video and work directory, its audio track and picture size,
every utterance as a `ReviewLine` (times, the language model's text and flags, each engine's
reading, why it is flagged), the owner's corrections, whether every line is shown, the line being
edited as a `Draft`, and the last notice.

## Boundaries

- Depends on: `job_model::outputs` (`Correction`, `Corrections`).
- Used by: `crate::line_review::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

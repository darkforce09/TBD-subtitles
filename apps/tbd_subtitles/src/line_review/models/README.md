# Line review models

The data Check Lines draws, with no rendering code.

## Contents

```text
apps/tbd_subtitles/src/line_review/models/
├── clip.rs     `Frame` (an RGBA preview frame with a serial of its own) and `Sound` (mix or voices)
├── mod.rs      the module list
└── session.rs  `ReviewSession`, `ReviewLine`, `Hypothesis`, `Draft`, `LineList`, `RunState`, `LineStatus`, `Parked`
```

## How it works

A `ReviewSession` holds one job's video and work directory, its audio track and picture size,
every utterance as a `ReviewLine` (times, the language model's text and flags, each engine's
reading, and each group the quality check put it in with why in plain words), the owner's
corrections, and what the owner did in this window: the line open, a `Draft` per edited line,
the list shown (`LineList`: To Check, Checked or All), the search, the group the list is
narrowed to, and each saved line's `RunState` (Saved, Updating, Updated or Failed), the newest
last. A line is flagged when it has a group, and `worth` a listen while flagged, corrected, or
taken back with its run not ended, so the set holds when a correction run drops the findings of
the lines it settles. `Parked` is what a closed review keeps: its drafts and its runs. `saved` gives a line's text and flags as saved (its
correction, else the language model's without `UNSURE`), `current` the draft over them, `kept`
whether its correction is the language model's reading unchanged (Looks Right), and `run_shown`
the run state the editor's header shows: the open line's, else the newest; `unchecked_fix` whether
its correction is a Fix It change the owner has not kept or undone. `LineStatus` names what a row
says: edited, changed by Fix It, kept, corrected, to check, or nothing.

## Boundaries

- Depends on: `job_model::outputs` (`Chosen`, `Correction`, `Corrections`);
  `crate::job_report::models::finding_group::LineGroup`.
- Used by: `crate::line_review::{services, ui}` and `crate::application`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

# Line review view

The review view, drawn from a borrowed `ReviewView`; it returns events and changes nothing.

## Contents

```text
apps/tbd_subtitles/src/line_review/ui/
├── mod.rs          the module list, `ReviewView` and `review_view_ui`
└── review_view.rs  the lines on the left, the line being edited on the right
```

## How it works

The left panel lists the flagged and corrected lines (every line with "Show every line"), each
with its time, id and text, marked ⚠ when flagged and ✎ when corrected. The right side shows the
line open for editing: its reasons, Play, Voices only or Stop, the clip's picture, every reading
with a Use button, the text, the flags (new speaker, narrator, song lyric, drop the line), Save and
time again, and Take the correction back. The picture's texture is kept in egui's memory and
uploaded only when a new frame arrives.

## Boundaries

- Depends on: `crate::line_review::{events, models, services::review_editing}`; `crate::core::ui`;
  `eframe`.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); Save is disabled while a full run of the
  job's video runs (the header of `review_view.rs`).

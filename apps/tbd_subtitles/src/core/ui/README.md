# Shared window look

The colours and sizes every feature's UI shares, so panels drawn by different features look alike.
It holds constants only; each feature draws its own widgets.

## Contents

```text
apps/tbd_subtitles/src/core/ui/
└── mod.rs  `MUTED_TEXT`: the grey of secondary text such as hints, empty states and file paths
```

## Boundaries

- Depends on: `eframe::egui::Color32`.
- Used by: `crate::application::window` for the central panel's note, and
  `crate::job_queue::ui::queue_panel` for the empty-queue hint.
- Rules: nothing here imports a feature or a composition module
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); a feature may use this folder though it
  may not use another feature's `ui`.

# Job report view

The report view, drawn from a borrowed `JobReport`; it returns events and changes nothing.

## Contents

```text
apps/tbd_subtitles/src/job_report/ui/
├── mod.rs          the module list and `report_view_ui`
└── report_view.rs  the verdict, the counts, the files, the findings and the steps
```

## How it works

The verdict comes first: passes, or each failed rule in red. Then the counts, the subtitle file
with Copy path, the buttons that open the video, its folder and `report.md`, the findings (time as
`h:mm:ss.d`, check, text, detail, and Review for a finding about one line) with a Review lines
button above them, and the steps (by their plain titles, with time, peak RAM and peak VRAM) in tables.

## Boundaries

- Depends on: `crate::job_report::{events, models}`; `crate::core::{format, steps, ui}`; `eframe`
  and `egui_extras`.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

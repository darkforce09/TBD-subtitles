# Line review panels

The line review panels, drawn from a borrowed view; they return events and change nothing. Their
code is not written yet: `mod.rs` holds only the module header.

## Contents

```text
apps/tbd_subtitles/src/line_review/ui/
└── mod.rs  the module header; no items
```

## Boundaries

- Depends on: nothing.
- Used by: nothing; `crate::line_review` declares the module and no code calls it.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

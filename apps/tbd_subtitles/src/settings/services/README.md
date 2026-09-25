# Settings services

The settings logic: loading, checking and changing its data, with no rendering code. Its code is
not written yet: `mod.rs` holds only the module header.

## Contents

```text
apps/tbd_subtitles/src/settings/services/
└── mod.rs  the module header; no items
```

## Boundaries

- Depends on: nothing.
- Used by: nothing; `crate::settings` declares the module and no code calls it.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`).

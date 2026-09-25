# Settings

The feature for the app's settings: the models folder and its downloads, the work folder, the
engines per stage, the language-model backend, the output format, the watch folders, and the GPU
check. Its code is not written yet: each of its files holds only a module header, and the app
reads no settings file.

## Contents

```text
apps/tbd_subtitles/src/settings/
├── mod.rs     the module tree and the feature's header
├── models/    the data the settings views draw, with no rendering code
├── services/  loading, checking and changing the settings, with no rendering code
└── ui/        the settings panels, drawn from a borrowed view
```

## How it works

The folder follows the layout every feature shares: `models/` and `services/` hold data and logic
free of egui, and `ui/` draws from a view the application lends it and returns events for the
application to apply after the frame. No code fills that layout yet, and the window does not show
the feature.

## Public surface

None: the folder declares `models`, `services` and `ui`, and none of them holds an item another
module can use.

## Boundaries

- Depends on: nothing.
- Used by: nothing; `apps/tbd_subtitles/src/main.rs` declares the module and no code calls it.
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`
  (`module_roots_and_documentation_describe_the_entire_source_tree`); `models/` and `services/`
  never name egui or eframe, and the feature never imports `application`, `cli` or another
  feature's `ui` (`dependency_boundaries_and_external_test_placement_are_enforced`); both tests
  are in `apps/tbd_subtitles/src/tests/architecture_rules.rs`.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the settings the window is built to edit.
- [System overview](/documentation/architecture/system_overview.md#configuration) — the TOML
  settings file and what it holds.

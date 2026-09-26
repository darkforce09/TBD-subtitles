# Settings panels

The settings page, drawn from the borrowed `SettingsPage`: the form over `settings.toml`, the
models and runtime list with its download, and the machine checks. It returns events and changes
nothing.

## Contents

```text
apps/tbd_subtitles/src/settings/ui/
├── machine_panel.rs   the models and runtime archives with their download, and the machine checks
├── mod.rs             the module list and `settings_page_ui`
└── settings_panel.rs  the page: the form, Save and Revert, the notice, then the machine panel
```

## How it works

`settings_page_ui` draws the form on a copy of the draft and sends it back whole as
`SettingsEvent::Edit` when a field changed: the models and work folders (a chooser and a way back
to the default), the work folder's size, the glossary, the separator, the Whisper model, the
`claude` model and process count, the cut score and the subtitle format. Save and Revert are
enabled while the draft differs from the file. The machine panel lists each model folder and
runtime archive with its size, and shows "on disk", "missing" or the download's progress bar,
then each check with a ✓, ⚠ or ✗ and what it found.

## Boundaries

- Depends on: `crate::settings` (`events`, `models`, `services::model_downloads::missing_bytes`);
  `crate::core` (`format`, `ui`); `job_model::job` for the choices; `eframe`.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); nothing is saved from here (the header of
  `settings_panel.rs`).

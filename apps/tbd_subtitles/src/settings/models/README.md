# Settings models

The data the settings views draw, with no rendering code: the owner's settings as kept in
`settings.toml`, the settings page's state, and what the page shows about this machine.

## Contents

```text
apps/tbd_subtitles/src/settings/models/
├── app_settings.rs  `AppSettings` with `Engines`, `LanguageModel` and `Backend`, and their defaults
├── machine.rs       `DownloadItem` and `ItemKind`, `Check` and `CheckState`
├── mod.rs           the module list
└── page.rs          `SettingsPage`, `Notice` and `DownloadProgress`
```

## How it works

`AppSettings` holds the models folder and the work folder (`None` for the defaults under the app
data folder), the glossary (`one_piece`, `none` or a file), the shot-cut score, the output format,
the engines (separator and Whisper model) and the language model (backend, model name and how many
run at once). Every struct takes its defaults for missing keys and refuses unknown ones. The one
backend is the `claude` CLI. `SettingsPage` holds the saved settings and the owner's draft, the
notice from the last save or download, the model folders and runtime archives with a running
download, the checks, and the work folder with its size.

## Boundaries

- Depends on: `job_model::job` (`OutputFormat`, `Separator`, `WhisperModel`) and `serde`.
- Used by: `crate::settings::{services, ui}`, `crate::application` and the `process` subcommand in
  `apps/tbd_subtitles/src/cli/process_command.rs`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); an unknown key is an error, never ignored
  (`an_unknown_key_is_an_error_naming_it` in `../services/tests/settings_file.rs`).

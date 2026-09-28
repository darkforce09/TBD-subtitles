# Settings models

The data the settings views draw, with no rendering code: the owner's settings as kept in
`settings.toml`, the Settings window's state with its tabs and the fields an error sits under,
and what the window shows about this machine.

## Contents

```text
apps/tbd_subtitles/src/settings/models/
├── app_settings.rs   `AppSettings` with `Engines`, `LanguageModel` and `Backend`, and their defaults
├── claude_models.rs  the `claude` models offered: name, label, tag and help line; `display_name`
├── machine.rs        `DownloadItem` and `ItemKind`, `Check` (with the folder it found), `CheckState`
├── mod.rs            the module list
└── page.rs           `SettingsPage`, `SettingsTab`, `Field`, `FieldError` and `DownloadProgress`
```

## How it works

`AppSettings` holds the models folder and the work folder (`None` for the defaults under the app
data folder), the glossary (`one_piece`, `none` or a file), the shot-cut score, the output format,
the engines (separator and Whisper model) and the language model (backend, the model a run asks,
the model Fix It asks, and how many run at once; Sonnet and Opus by default). Every struct takes
its defaults for missing keys and refuses unknown ones. The one backend is the `claude` CLI. `SettingsPage` holds the settings as the file has them (`saved`; an
edit is written at once, so there is no draft), the `FieldError` of the last edit that was
refused (its `Field` and why), why the file could not be read, the saved glossary's count of
names, the model folders and runtime archives with a running download and when a download last
brought everything onto disk, the checks, and the models and work folders with their sizes.
`SettingsTab` names the Settings window's four tabs (General, Engines, Models, This Computer) and
their titles. `DownloadProgress` names the item downloading now by its id, with its bytes held,
and the bytes of the whole download. `CLAUDE_MODELS` lists the `claude` models both lists offer
(Sonnet, Opus, Fable, Haiku) with the help line of each; `display_name` gives a model's name as
the window writes it, such as "Claude Opus", for the Fix It button and its messages.

## Boundaries

- Depends on: `job_model::job` (`OutputFormat`, `Separator`, `WhisperModel`) and `serde`.
- Used by: `crate::settings::{services, ui}`, `crate::application` and the `process` subcommand in
  `apps/tbd_subtitles/src/cli/process_command.rs`.
- Rules: nothing here names egui or eframe
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); an unknown key is an error, never ignored
  (`an_unknown_key_is_an_error_naming_it` in `../services/tests/settings_file.rs`).

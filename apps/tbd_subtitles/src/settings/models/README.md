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
└── page.rs           `SettingsPage`, `SettingsTab`, `Field` and its error, downloads, right-click, the library
```

## How it works

`AppSettings` holds the models folder and the work folder (`None` for the defaults under the app
data folder), the glossary (`one_piece`, `none` or a file), the shot-cut score, the output format,
the watch folders (none by default, and an empty list is not written), the engines (separator and Whisper model) and the language model (backend, the model a run asks,
the model Fix It asks, and how many run at once; Sonnet and Opus by default; how many `claude`
calls Fix It makes at once across every video, 32 by default; and whether Fix It starts on each
video when its full run finishes, off by default), and the on-screen text settings (translation,
Claude fallback, a reference folder, and Replace text in the video, on for new jobs). Every struct
takes its defaults for missing keys, so a file written before a setting existed still loads, and
refuses unknown ones; a file with on-screen text settings but no `localized_video` key, written
before the localized video existed, replaces text in the video, while a saved `false` stays off. The one backend is the `claude` CLI. `SettingsPage` holds the settings as the file has them (`saved`; an
edit is written at once, so there is no draft), the `FieldError` of the last edit that was
refused (its `Field` and why), why the file could not be read, the saved glossary's count of
names, the model folders and runtime archives with a running download and when a download last
brought everything onto disk, the checks, the models and work folders with their sizes, the
`SignLibrary` (its signs and bytes once measured, its last error, whether a Clear waits for
confirmation or runs), and the `RightClickEntry`: whether Dolphin's "Generate subtitles" entry
is written (not installed until the application writes it, installed at its path, or failed with
the reason).
`SettingsTab` names the Settings window's five tabs (General, Automation, Engines, Models, This
Computer) and their titles. `DownloadProgress` names the item downloading now by its id, with its bytes held,
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

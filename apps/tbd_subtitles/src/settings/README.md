# Settings

The feature for the app's settings: the models folder and its downloads, the work folder, the
glossary, the engines per stage, the language-model backend, the cut score, the output format,
the watch folders and the on-screen text, kept in `~/.config/tbd-subtitles/settings.toml`, the
sign library's size and clearing, and the check of this machine. The window's Settings window
edits them in six tabs, each change written as it is made,
and a banner under the toolbar says when a model is missing; the `process` subcommand reads the
same file.

## Contents

```text
apps/tbd_subtitles/src/settings/
├── events.rs  `SettingsEvent` and `PathField`: what the Settings window and the banner ask for
├── mod.rs     the module tree and the feature's header
├── models/    the settings, the window's state and tabs, and the machine data; no rendering code
├── services/  the file, the job settings, edits, downloads, the models list, checks and sizes
└── ui/        the Settings window's tabs and the models banner, drawn from a borrowed view
```

## How it works

The folder follows the layout every feature shares: `models/` and `services/` hold data and logic
free of egui, and `ui/` draws from a view the application lends it and returns events for the
application to apply after the frame. `models/app_settings.rs` holds every setting with its
default and `models/page.rs` the window's state: the file's settings, the error of an edit that
was refused, the glossary's names, the models list, a running download by item id, the checks,
the sizes of the models and work folders, and whether Dolphin's right-click entry is written
(`RightClickEntry`, which the application sets); it names the tabs (`SettingsTab`) and the fields
an error sits under (`Field`, `FieldError`). `services/` reads and writes the file, turns the
settings into a job's `JobSettings`, applies an edit (written at once when it can make a job,
refused with an error under its field otherwise, and saying what it made stale), lists and
downloads the models and runtime archives, turns them into the Models tab's rows, what is missing
and the banner, runs the machine checks and measures a folder, the slow parts each on a thread of
its own. The watch folders are a setting here (a chosen folder is added once, as its canonical
path; a folder that is not there is kept); watching them belongs to the automation feature.

## Public surface

- `models::app_settings::AppSettings` and `services::{settings_file, job_settings}`, for the
  `process` subcommand and the application.
- `events::SettingsEvent`, `models::page::{SettingsPage, SettingsTab}`,
  `ui::{settings_window_ui, models_banner_ui}` and the other services, for the application.

## Boundaries

- Depends on: `job_model::job`, `stages::adjudication::glossary`, `pipeline::{models, work_dir,
  measure, library}`, `inference::{model_store, cuda_runtime, llm::claude_cli}`, `child_process`,
  `crate::core`, `serde`, `toml`, `anyhow` and `eframe` (in `ui/` only).
- Used by: `apps/tbd_subtitles/src/cli/process_command.rs` and `crate::application`.
- Rules: the folder keeps `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`
  (`module_roots_and_documentation_describe_the_entire_source_tree`); `models/` and `services/`
  never name egui or eframe, and the feature never imports `application`, `cli` or another
  feature's `ui` (`dependency_boundaries_and_external_test_placement_are_enforced`); both tests
  are in `apps/tbd_subtitles/src/tests/architecture_rules.rs`.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the settings the window edits.
- [System overview](/documentation/architecture/system_overview.md#configuration) — the TOML
  settings file and what it holds.
- [Settings apply as they change](/documentation/decisions/desktop_gui.md#2026-09-28--settings-apply-as-they-change)
  — why there is no Save.

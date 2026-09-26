# Settings

The feature for the app's settings: the models folder and its downloads, the work folder, the
glossary, the engines per stage, the language-model backend, the cut score and the output format,
kept in `~/.config/tbd-subtitles/settings.toml`, and the check of this machine. The window's
Settings page edits them; the `process` subcommand reads the same file.

## Contents

```text
apps/tbd_subtitles/src/settings/
├── events.rs  `SettingsEvent` and `PathField`: what the settings page asks for
├── mod.rs     the module tree and the feature's header
├── models/    the settings, the page's state and the machine data, with no rendering code
├── services/  the file, the job settings, saving, downloads, checks and sizes; no rendering code
└── ui/        the settings page, drawn from a borrowed view
```

## How it works

The folder follows the layout every feature shares: `models/` and `services/` hold data and logic
free of egui, and `ui/` draws from a view the application lends it and returns events for the
application to apply after the frame. `models/app_settings.rs` holds every setting with its
default and `models/page.rs` the page: the file's settings, the draft, the models list, a running
download, the checks and the work folder's size. `services/` reads and writes the file, turns the
settings into a job's `JobSettings`, saves the draft only when it can make a job, lists and
downloads the models and runtime archives, runs the machine checks and measures the work folder,
the slow parts each on a thread of its own. Watch folders belong to the automation feature.

## Public surface

- `models::app_settings::AppSettings` and `services::{settings_file, job_settings}`, for the
  `process` subcommand and the application.
- `events::SettingsEvent`, `models::page::SettingsPage`, `ui::settings_page_ui` and the other
  services, for the application.

## Boundaries

- Depends on: `job_model::job`, `stages::adjudication::glossary`, `pipeline::{models, work_dir,
  measure}`, `inference::{model_store, cuda_runtime}`, `child_process`, `crate::core`, `serde`,
  `toml`, `anyhow` and `eframe` (in `ui/` only).
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

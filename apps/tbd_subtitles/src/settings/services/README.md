# Settings services

The settings logic, with no rendering code: where `settings.toml` lives, reading and writing it,
turning the owner's settings into what a job runs with, saving the page's draft, and the machine
side: the models and runtime a job needs and their download, the checks of this machine, and the
work folder's size.

## Contents

```text
apps/tbd_subtitles/src/settings/services/
├── job_settings.rs     the job settings, the models folder and the work folder the settings name
├── model_downloads.rs  the models and runtime archives a job needs, and their download on a thread
├── mod.rs              the module list
├── page_editing.rs     saving the draft when it can make a job, and reverting it
├── settings_file.rs    the file's path, `load`, `parse`, `render` and `save`, and `SettingsError`
├── system_check.rs     the GPU, CUDA runtime, FFmpeg, ffprobe, `claude` and Whisper worker checks
├── tests/              unit tests for each file here
└── work_folder.rs      the work folder's size, measured on a thread
```

## How it works

`settings_file::default_path` is `tbd-subtitles/settings.toml` under `XDG_CONFIG_HOME`, else
`~/.config`. `load` reads it: a missing file is the defaults, and anything else that fails, an
unreadable file, broken TOML, an unknown key or a bad value, is a `SettingsError` naming the file
and the key. `save` writes the whole file through a part file and a rename.
`job_settings::job_settings` reads the glossary the settings name and fills a `JobSettings`;
`models_dir` and `work_root` give the named folders, else the defaults of `pipeline`.
`page_editing::save` writes the draft only when it makes a job's settings.

`model_downloads::plan` lists the model folders `pipeline::models::required` names and the runtime
archives the workers load (the CUDA 13 libraries and ONNX Runtime; not the build-only toolkit),
each with its size and whether it is on disk; a complete runtime found beside the binary or in the
runtime folder counts every archive as present. `start` downloads the missing ones on a thread
through `inference::model_store`, each pinned by size and SHA-256, reporting bytes held per item;
`stop` ends it after the current block and the part file stays for the next attempt.
`system_check::run_all` reads the GPU through NVML (name, driver, free memory against the 5632
MiB a GPU step needs), locates the CUDA runtime, asks FFmpeg, ffprobe and `claude` for their
versions, looks for FFmpeg's `pulse` output (clip sound), and looks for `tbd-subtitles-ggml`
beside the binary. `work_folder::size` sums every file under the work folder.

## Boundaries

- Depends on: `crate::settings::models`; `crate::core::background::Wake`; `toml` and `serde`;
  `job_model::job`; `stages::adjudication::glossary`; `pipeline::{models, work_dir, measure}`;
  `inference::{model_store, cuda_runtime}`; `child_process`; `anyhow`.
- Used by: the `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs`;
  `crate::application`; `crate::settings::ui` (`missing_bytes`).
- Rules:
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - an empty file is the measured defaults, and every key is read
    (`an_empty_file_is_the_measured_defaults`, `every_key_is_read` in `tests/settings_file.rs`);
  - an unknown key or a bad value is an error naming it, and a missing file is the defaults
    (`an_unknown_key_is_an_error_naming_it`, `a_bad_value_is_an_error_naming_it`,
    `a_missing_file_is_the_defaults_and_a_broken_one_an_error`);
  - saved settings load back unchanged, and a draft that cannot make a job is not saved
    (`saved_settings_load_back_unchanged`; `a_draft_that_cannot_make_a_job_is_not_saved` in
    `tests/page_editing.rs`);
  - every setting reaches the job, and a missing glossary file is an error naming it
    (`every_setting_reaches_the_job`, `a_missing_glossary_file_is_an_error_naming_it` in
    `tests/job_settings.rs`);
  - the build-only toolkit is never downloaded for the app
    (`the_build_toolkit_is_not_downloaded_for_the_app` in `tests/model_downloads.rs`);
  - a machine without the NVIDIA driver fails the GPU check, never passes it
    (`no_driver_is_a_failure_never_a_pass` in `tests/system_check.rs`).

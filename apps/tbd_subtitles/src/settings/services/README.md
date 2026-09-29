# Settings services

The settings logic, with no rendering code: where `settings.toml` lives, reading and writing it,
turning the owner's settings into what a job runs with, applying an edit of the Settings window,
and the machine side: the models and runtime a job needs, their download and how the Models tab
and the banner show them, the checks of this machine, and the folders' sizes.

## Contents

```text
apps/tbd_subtitles/src/settings/services/
├── job_settings.rs     the job settings, the models folder and the work folder the settings name
├── model_downloads.rs  the models and runtime archives a job needs, their download, events by id
├── model_list.rs       the Models tab's rows, what is missing with its words, and the banner
├── mod.rs              the module list
├── page_editing.rs     applying an edit at once or refusing it; what it stales; adding a watch folder
├── settings_file.rs    the file's path, `load`, `parse`, `render` and `save`, and `SettingsError`
├── system_check.rs     the GPU, CUDA runtime, FFmpeg, ffprobe, `claude` and Whisper worker checks
├── tests/              unit tests for each file here
└── work_folder.rs      a folder's size (the work or the models folder), measured on a thread
```

## How it works

`settings_file::default_path` is `tbd-subtitles/settings.toml` under `XDG_CONFIG_HOME`, else
`~/.config`. `load` reads it: a missing file is the defaults, and anything else that fails, an
unreadable file, broken TOML, an unknown key or a bad value, is a `SettingsError` naming the file
and the key. `save` writes the whole file through a part file and a rename.
`job_settings::job_settings` reads the glossary the settings name and fills a `JobSettings`;
`job_settings::glossary_name` names the glossary as Fix It tells the model (`one_piece`, `none`,
or a glossary file's name without its extension);
`models_dir` and `work_root` give the named folders, else the defaults of `pipeline`.
`page_editing::apply` takes the saved settings with one field changed and writes them at once,
returning what they made stale (the models list when the models folder or an engine changed, the
models folder's size, the work folder's size; never the machine checks). It refuses, writing
nothing, with a `FieldError` naming the field and why: a number out of its range (`PROCESSES`
1–16, `FIX_CALLS` 1–100, `CUT_SCORES` 1–100, and never one that is not finite), the models
folder while a download runs ("Stop the download first."), or a new glossary that cannot be read
("Can't use names.json: … The glossary was not changed."). The glossary is read only when it changes, so an unreadable
one blocks no other edit, and its error stays under it until it changes. Before the first write
over a settings file that could not be read, the file is kept beside itself as
`settings.toml.broken` (`Applied::kept`). Every edit keeps each watch folder once, in its order,
and a folder that is not there is written all the same (a drive may be unmounted);
`with_watch_folder` adds a chosen folder to the end as its canonical path (as given when it has
none), unless it is watched already. `read_glossary` counts the saved glossary's names, or
puts why it cannot be read under the glossary.

`model_downloads::plan` lists the model folders `pipeline::models::required` names and the runtime
archives the workers load (the CUDA 13 libraries and ONNX Runtime; not the build-only toolkit),
each with its size and whether it is on disk; a complete runtime found beside the binary or in the
runtime folder counts every archive as present. `start` downloads the missing ones on a thread
through `inference::model_store`, each pinned by size and SHA-256, reporting bytes held and each
item finished by its id, and how it ended (done, stopped or failed); `stop` ends it after the
current block and the part file stays for the next attempt. `begun` is the progress as a download
starts, and `fold` folds an event into the page, finding the item by id, so a list planned again
during a download (another engine chosen) is marked right. `model_list::rows` gives one row per
model, then the 13 CUDA archives as one row ("CUDA and cuDNN (13 archives)"), then ONNX Runtime,
each on disk, missing or downloading with its share; `missing` counts the missing rows, models and
runtime libraries apart, with their bytes and their headline ("5 models and 2 runtime libraries
are missing (10.7 GiB)"); `banner` is the banner under the toolbar: the download while one runs,
else what is missing, else for 4 s after a download that brought everything onto disk.
`system_check::run_all` reads the GPU through NVML (name, driver, free memory against the 5632
MiB a GPU step needs), locates the CUDA runtime (the check `CUDA_RUNTIME`, with the folder it was
found in as its `path`), asks FFmpeg, ffprobe and `claude` for their versions, looks for FFmpeg's
`pulse` output (clip sound), and looks for `tbd-subtitles-ggml` beside the binary.
`work_folder::size` sums every file under a folder.

## Boundaries

- Depends on: `crate::settings::models`; `crate::core::{background::Wake, format}`; `toml` and
  `serde`; `job_model::job`; `stages::adjudication::glossary`; `pipeline::{models, work_dir,
  measure}`; `inference::{model_store, cuda_runtime}`; `child_process`; `anyhow`.
- Used by: the `process` subcommand in `apps/tbd_subtitles/src/cli/process_command.rs`;
  `crate::application`; `crate::settings::ui` (`model_list`, `system_check::CUDA_RUNTIME`,
  `page_editing::{PROCESSES, FIX_CALLS, CUT_SCORES}`).
- Rules:
  - nothing here names egui or eframe
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - an empty file is the measured defaults, every key is read, and a file from before Fix It's
    calls at once and its switch makes 32 calls and waits to be asked
    (`an_empty_file_is_the_measured_defaults`, `every_key_is_read`,
    `a_file_from_before_fix_it_ran_at_scale_makes_32_calls_and_waits_to_be_asked` in
    `tests/settings_file.rs`);
  - an unknown key or a bad value is an error naming it, and a missing file is the defaults
    (`an_unknown_key_is_an_error_naming_it`, `a_bad_value_is_an_error_naming_it`,
    `a_missing_file_is_the_defaults_and_a_broken_one_an_error`);
  - watch folders load back, an empty list is not written, and a file without them watches none
    (`watch_folders_load_back_and_an_empty_list_is_not_written`,
    `a_file_without_watch_folders_watches_none`); watch folders are a field of their own, a
    chosen one is added once as its canonical path, and one that is missing is written once
    (`watch_folders_are_a_field_of_their_own_that_makes_nothing_stale`,
    `a_chosen_watch_folder_is_added_once_as_its_canonical_path`,
    `a_watch_folder_is_written_once_even_when_it_is_missing` in `tests/page_editing.rs`);
  - saved settings load back unchanged (`saved_settings_load_back_unchanged`); a valid edit is
    written at once, a bad glossary is not written and names its field, an unreadable saved
    glossary blocks no other edit and keeps its error, numbers out of range or not finite are
    never written, the models folder stays while a download runs, an unreadable settings file is
    kept before the first write, and only the models folder and the engines make the models list
    stale (`a_valid_edit_is_written_at_once`, `a_bad_glossary_is_not_written_and_names_its_field`,
    `an_unreadable_saved_glossary_blocks_no_other_edit_and_its_error_stays`,
    `numbers_out_of_range_or_not_finite_are_never_written`,
    `claude_calls_at_once_stay_from_1_to_100`,
    `fix_it_s_calls_and_its_switch_are_fields_of_their_own_that_make_nothing_stale`,
    `the_models_folder_stays_while_a_download_runs`,
    `an_unreadable_settings_file_is_kept_before_the_first_write`,
    `only_the_models_folder_and_the_engines_make_the_models_list_stale` in
    `tests/page_editing.rs`);
  - every setting reaches the job, and a missing glossary file is an error naming it
    (`every_setting_reaches_the_job`, `a_missing_glossary_file_is_an_error_naming_it` in
    `tests/job_settings.rs`);
  - the build-only toolkit is never downloaded for the app, and a list planned again during a
    download is marked by id (`the_build_toolkit_is_not_downloaded_for_the_app`,
    `a_list_planned_again_during_a_download_is_marked_by_id` in `tests/model_downloads.rs`);
  - what is missing names models and runtime libraries apart
    (`missing_names_models_and_runtime_libraries_apart` in `tests/model_list.rs`);
  - a machine without the NVIDIA driver fails the GPU check, never passes it
    (`no_driver_is_a_failure_never_a_pass` in `tests/system_check.rs`).

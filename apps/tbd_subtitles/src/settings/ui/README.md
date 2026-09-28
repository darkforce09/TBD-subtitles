# Settings window and models banner

The Settings window's content and the banner under the toolbar, drawn from the borrowed
`SettingsPage`: the tab bar General | Engines | Models | This Computer, the open tab, the footer,
and the banner while a model is missing or downloads. They return events and change nothing.

## Contents

```text
apps/tbd_subtitles/src/settings/ui/
├── engines_tab.rs      vocal separation, second speech engine, language model, processes, cut score
├── form.rs             the forms' rows, help and error lines, divider, path well, list, field, stepper
├── general_tab.rs      the models and work folders with their sizes, subtitle format, glossary
├── machine_tab.rs      each machine check with its mark and detail, and Check Again
├── mod.rs              the module list and the two entry points
├── models_banner.rs    the banner under the toolbar: missing, downloading, or all on disk
├── models_tab.rs       the models and runtime table, and Download Missing, Stop or all on disk
├── settings_window.rs  `settings_window_ui`: the tab bar, the open tab, the footer
└── tests/              unit tests of the typed numbers and the home written as `~`
```

## How it works

`settings_window_ui` draws the tab bar on the toolbar grey (an icon over each name, at least
88 px wide, the open tab in the accent tint), the open tab in a scrolling body 24 px from the
sides, and the footer "Changes save as you make them. They apply to videos that haven't
started." The General and Engines tabs are forms: a right-aligned 170 px label beside its
controls, 11.5 px grey help under them, and a refused edit's error in red with a cross under its
field. Each change is an `SettingsEvent::Edit` of the saved settings with that one change, which
the application writes at once: lists and the segmented format on a choice, the stepper's arrows
on each press, and a stepper's typed number on Enter, when the field loses the focus, or when
the window closes with it typed (the text being typed lives in egui's memory while the field has
the focus and goes once it is sent); a typed number that is not finite or not in its range is
dropped. General shows the models folder with its size on disk, the work folder
with its size and Open, Choose… and, for a folder set by hand, Default (the models folder's
buttons are off while a download runs: "Stop the download first"); each folder on one line, the
home as `~` and cut in the middle when too long, the whole path on hover; SRT | WebVTT | ASS; and
the glossary with its count of names. Engines shows the separator, the Whisper model and the
`claude` model (Sonnet, Opus, Fable or Haiku, or a name kept in `settings.toml`) in lists as wide
as their column, with help that follows the choice, processes at once (1–16) and the shot cut
score (1–100). Models lists the rows of
`model_list::rows` (Name, Kind, Size, Status: On disk, Missing, or a bar with its share), then
Download Missing (size), or Stop with "Downloading … of …. A stopped file resumes next time.",
or "Everything a job needs is on disk." This Computer lists each check with ✓, ⚠ or ✗ (a spinner
while they run again), its name and what it found; the CUDA runtime's folder stays on one line
like the General tab's, or reads "in the models folder" when it is inside it; a failed CUDA
runtime links to the Models tab
("Download it in Models"); Check Again is off while checks run. `models_banner_ui` draws the
banner `model_list::banner` chose, in the orange tint while something is missing or downloads,
green when all is on disk: the headline, a grey line and, while downloading, a bar at most 420 px
wide, with Details… (the Models tab) and Download, or Stop.

## Boundaries

- Depends on: `crate::settings` (`events`, `models`, `services::{model_list,
  system_check::CUDA_RUNTIME, page_editing::{PROCESSES, CUT_SCORES}}`); `crate::core` (`format`,
  `ui`); `job_model::job` for the choices;
  `eframe`.
- Used by: `crate::application::feature_views`.
- Rules: no module outside the feature but `application` may import this folder
  (`dependency_boundaries_and_external_test_placement_are_enforced` in
  `apps/tbd_subtitles/src/tests/architecture_rules.rs`); nothing is written from here (the header
  of `settings_window.rs`); the tabs, the footer and the banner show their words
  (`each_settings_tab_shows_its_settings_under_the_tab_bar_and_over_the_footer`,
  `the_banner_says_what_is_missing_and_details_opens_the_models_tab` in
  `apps/tbd_subtitles/src/application/tests/rendering_settings.rs`); a typed number must be finite
  and in range, and the home reads as `~` (`a_typed_number_must_be_finite_and_in_range`,
  `the_home_folder_reads_as_a_tilde` in `tests/form.rs`); a folder reads from home on one line
  (`folders_read_from_home_on_one_line_and_a_runtime_in_the_models_folder_says_so` in
  `rendering_settings.rs`).

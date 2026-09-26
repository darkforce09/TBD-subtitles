# Application actions

Applying the actions a frame collected, one module per feature, and folding each feature's thread
answers in before the next frame.

## Contents

```text
apps/tbd_subtitles/src/application/actions/
├── mod.rs       the module list and the re-exports `application` uses
└── settings.rs  the settings page as the window opens, its actions, and its threads' answers
```

## How it works

`settings.rs` builds the settings page from the settings file (the defaults, with the reason, when
the file cannot be read) and the models its settings need. Its actions change the draft, save it
through `settings::services::page_editing`, open the desktop's chooser for a path setting, start or
stop the model download, and run the machine checks again. `poll_settings` folds the download's
progress, the checks and the work folder's size into the page; a finished download lists the
models again.

## Boundaries

- Depends on: `crate::settings` (events, models, services); `crate::core::portal`;
  `crate::application` (`TbdSubtitlesApp`, `Environment`, `background::Chooser`).
- Used by: `crate::application`, in `apply` and `poll`.
- Rules: one download and one check run at a time, and a chooser's answer changes only the draft
  (the header of `settings.rs`).

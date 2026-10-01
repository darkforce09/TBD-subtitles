# Models a job needs

The one list of the models a job loads: each step's model folder, the folders a job's settings
need, and which of them the models folder lacks.

## Contents

```text
crates/pipeline/src/models/
├── mod.rs  `separator_model`, `whisper_model`, `required`, `missing` and `default_dir`
└── tests/  unit tests: every required folder is pinned, the settings choose the folders
```

## How it works

`required` lists, in the order the steps load them, the separation model the settings choose,
Parakeet-TDT, the Whisper model the settings choose, CED-base and Parakeet-CTC; with on-screen
text on, also PP-OCRv5, manga-ocr and Qwen3.5-4B, and with the localized video on, LaMa and the
Latin fonts. `missing` keeps those that `inference::model_store::is_complete` does not find whole
in the given folder. The separation and speech tasks open their models through `separator_model`
and `whisper_model`, so the list and the tasks never disagree. `default_dir` is the model store's
folder under the app data folder.

## Boundaries

- Depends on: `inference::model_store` and the model ids in `inference::onnx`; `job_model`
  (`JobSettings`, `Separator`, `WhisperModel`); `crate::error`.
- Used by: `crate::tasks` (`media.rs`, `speech.rs`); the app's `process` subcommand and its
  window, which check and download the missing models before a job starts.
- Rules: every required folder is pinned in the manifest, so it can be downloaded with a checksum
  (`every_required_model_is_pinned_in_the_manifest` in `tests/models.rs`), and the settings choose
  the separator and Whisper folders (`the_settings_choose_the_separator_and_whisper_folders`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the model files and why each was
  chosen.

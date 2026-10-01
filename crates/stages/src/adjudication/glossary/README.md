# Series glossaries

The names and terms the language model spells right and the novelty check accepts as heard. The
One Piece glossary is built into the binary and is the default; `mod.rs` also reads a glossary
file the owner passes.

## Contents

```text
crates/stages/src/adjudication/glossary/
├── mod.rs          the built-in glossary, the reader for glossary files, and the borrowed form
├── one_piece.json  the One Piece glossary: Dressrosa names, places, attacks and alias traps
└── tests/          unit tests for the built-in glossary and the file format
```

## Format

- Encoding: UTF-8 JSON, one glossary per file, named after the series in snake_case.
- Schema: a JSON array of strings, one name or term each, such as `"Trafalgar Law"` or
  `"Kin'emon"`. `parse` trims every entry, drops empty ones, and refuses anything that is not an
  array of strings.
- Adding a file: a built-in glossary goes here with an `include_str!` constant and a function
  beside `one_piece` in `mod.rs`, and `cargo test -p stages` checks it parses; a glossary for one
  run needs no code, only a file passed to `tbd-subtitles process --glossary <file>`.

## Producers and consumers

- Producers: people; no tool writes these files.
- Consumers: `mod.rs`, which embeds `one_piece.json` at compile time; the app's job settings in
  `apps/tbd_subtitles/src/settings/services/job_settings.rs`, which put the chosen glossary
  (`one_piece`, `none` or a file, from the settings or the `process` subcommand's `--glossary`)
  into `JobSettings::glossary`, from where the pipeline's language-model steps pass it to the
  prompts and the checks; the stack spike tool in `tools/stack_spike/`, which borrows it through
  `as_strs`; and the visual validation pilot in `tools/visual_validation/`.

## Boundaries

- Depends on: `serde_json`, to read the array.
- Used by: `apps/tbd_subtitles/src/settings/services/job_settings.rs`, `tools/stack_spike/` and
  `tools/visual_validation/`.
- Rules:
  - the built-in glossary parses and holds the main names
    (`the_built_in_glossary_parses_and_holds_the_main_names` in `tests/glossary.rs`);
  - a glossary file is an array of strings, trimmed, with no empty entry
    (`a_glossary_file_must_be_an_array_of_strings`);
  - the glossary is part of the language-model steps' settings, so a changed glossary re-runs them
    (`settings` in `crates/pipeline/src/graph/mod.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#6-adjudication) — how the glossary reaches
  the prompt and the novelty check.

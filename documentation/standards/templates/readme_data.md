**Status:** live

# README template: data

**When to use:** a folder of files that code reads rather than runs: a model manifest, the JSON
Schemas of the language model's answers, or a table of sound-cue labels a stage reads. Test
fixtures live under a `tests/` folder, which is exempt and needs no README. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the data kind adds Format, and Producers and consumers.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. A folder of one
homogeneous collection lists it as one glob line in Contents. Data outside `tests/` is TOML or
JSON: `cargo gates language-bans` allows no other kind there, and admits subtitle files only as
fixtures inside a `tests/fixtures/` folder.

````markdown
# <What the data is, in plain words: no path, no backticks>

<One to three sentences: what the data is for and what reads it.>

## Contents

```text
<repository path of the folder>/
├── <child folder>/  <what it holds: a lowercase phrase, no closing period>
└── <file or glob>   <what it holds>
```

## How it works

<How the files are read and checked: the code or test that consumes them, when it reads them
(compile time, start-up, a test run), and the invariants that hold across files. Leave the section
out when the folder has no child folders besides exempt ones and at most three files.>

## Format

- Encoding: <file type and encoding, and the naming convention>
- Schema: <the fields and their types, or the structure the reading code expects, with the file
  that defines it>
- Adding a file: <where it goes, what it must hold, and the command that checks it>

## Producers and consumers

- Producers: <what writes the files (a person, a tool, a stage), with its path>
- Consumers: <what reads them (modules, tests), each with its path>

## Boundaries

- Depends on: <the types and sources the data must agree with>
- Used by: <everything outside the folder that reads it, found with git grep>
- Rules: <the invariants a change must keep: stable names, pinned sources, generated files, and
  the test or gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

This sample is illustrative: it describes the model manifest of the
[system overview](/documentation/architecture/system_overview.md#models) as a folder of one TOML
file per model, while the model store keeps its manifest as Rust code in
`crates/inference/src/model_store/manifest.rs`. The folder, its files, its fields and its test do
not exist; the sample shows the shape a data README takes. The repository's real data folder, the
series glossaries in `crates/stages/src/adjudication/glossary/`, has its own README written from
its files. The sample sits in a fenced block, so no gate
reads it as a README or checks its paths.

````markdown
# Model manifest

The pinned source of every model file the app downloads: where it comes from, its checksum, its
size and its licence. The model store compiles the manifest into the binary and checks every
downloaded file against it.

## Contents

```text
crates/inference/src/model_store/manifest/
└── *.toml  one model each, named after the model id, such as `parakeet_tdt_0_6b_v2.toml`
```

## Format

- Encoding: UTF-8 TOML, one model per file, named `<model id>.toml` in snake_case; the file name
  is the id.
- Schema: the fields of `ManifestEntry` in `crates/inference/src/model_store/manifest_entry.rs`:
  `capability` (`asr`, `vad`, `separation`, `alignment`, `sound_events` or `llm`), `format`
  (`onnx`, `gguf` or `safetensors`), `files` (a list of `url`, `sha256` and `bytes`, each `url`
  pinned to a commit revision, never a branch), and `licence`.
- Adding a file: copy an existing entry, fill every field from the model's release page, then run
  `cargo test -p inference`, whose manifest test parses every file and checks that each `url` is
  pinned.

## Producers and consumers

- Producers: people; no tool writes these files. Models are downloaded already exported, never
  converted, so an entry names only published files.
- Consumers: `crates/inference/src/model_store/mod.rs`, which embeds every file with
  `include_str!` and downloads and verifies a model on first use; the settings feature of the app,
  which lists each model with its size and download status.

## Boundaries

- Depends on: `ManifestEntry`, which the files must parse into, and the release pages each `url`
  points at.
- Used by: the model store, and through it every GPU stage that loads a model.
- Rules: an entry's `url` is pinned and its `sha256` matches the file, so a download is the same
  bytes on every machine; a model id never changes once jobs have used it, because job records
  name models by id.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the model files each capability
  uses, and where they are published.
````

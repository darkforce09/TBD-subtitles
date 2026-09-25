**Status:** live

# README template: leaf

**When to use:** a folder with no child folders besides exempt ones (`tests/`, `generated/`), such
as a Rust module that holds only source files. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the leaf kind adds no sections of its own.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. A leaf of at
most three files, README.md not counted, may leave out How it works.

````markdown
# <What the folder holds, in plain words: no path, no backticks>

<One to three sentences: what this folder is for.>

## Contents

```text
<repository path of the folder>/
├── <file name or glob>  <what it is for: a lowercase phrase, no closing period>
└── tests/               <what the unit tests cover>
```

## How it works

<How the files work together: the flow through them, the main types, the invariants that span
files. Leave the section out when the folder holds at most three files.>

## Boundaries

- Depends on: <the modules, crates and files this folder uses, read from its imports>
- Used by: <every user outside the folder, found with git grep; "nothing" when none>
- Rules: <the invariants a change here must keep, and the test or gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers; leave the section
  out when no document goes deeper>
````

## Worked sample

Written from `crates/job_model/src/stage/`, a leaf of two source files and a `tests/` folder, so
it leaves out How it works. The sample sits in a fenced block, so no gate reads it as a README; the
folder's own README.md is written from the same code and may differ.

````markdown
# Stage names

The one list of the pipeline's [stages](/documentation/glossary.md#stage), in run order, with the
name each carries on the command line, in file names and in JSON, and which of them run in a
[worker process](/documentation/glossary.md#worker-process).

## Contents

```text
crates/job_model/src/stage/
├── mod.rs         the module tree and the re-export of the stage name
├── stage_name.rs  every stage in run order, with its command-line and JSON name
└── tests/         unit tests for the stage names
```

## Boundaries

- Depends on: `serde`, whose derive gives each stage its snake_case JSON name.
- Used by: `crates/job_model/src/lib.rs`, which re-exports `StageName`; the app's command line in
  `apps/tbd_subtitles/src/cli/`, whose `worker` subcommand parses a stage name and accepts only the
  stages that run in a worker process.
- Rules:
  - `StageName::ALL` lists every stage once, in run order, and each name parses back to its own
    stage (`every_stage_is_listed_once_and_parses_back_to_itself` in `tests/stage_name.rs`);
  - the JSON names equal the command-line names, so a resumed job reads what an earlier run wrote
    (`json_names_match_the_command_line_names`);
  - sound events run before adjudication, which chooses the sound cues
    (`sound_events_come_before_adjudication_which_chooses_the_cues`);
  - only separation, speech recognition, sound events, adjudication and alignment run in a worker
    process (`only_model_stages_run_in_a_worker`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does, in order.
````

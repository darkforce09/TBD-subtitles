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

Written from `crates/job_model/src/stage/`, a leaf of three source files and a `tests/` folder. It
may leave out How it works and keeps a short one to say how stages and steps relate. The sample
sits in a fenced block, so no gate reads it as a README; the folder's own README.md is written from
the same code and may differ.

````markdown
# Stage and step names

Every [stage](/documentation/glossary.md#stage) of the pipeline and every step the stages are made
of, in run order, with the one name each carries on the command line, in the job database's keys
and in JSON.

## Contents

```text
crates/job_model/src/stage/
├── mod.rs         the module tree and the re-exports of the stage and step names
├── stage_name.rs  every stage in run order, with its name and whether it runs in a worker
├── step_name.rs   every step in run order, with its name and the stage it belongs to
└── tests/         unit tests for the stage and step names, JSON and rkyv
```

## How it works

`StageName::ALL` lists the thirteen stages in run order; `runs_in_worker` says which of them run
in a [worker process](/documentation/glossary.md#worker-process). A stage runs as one or more
steps: `StepName::ALL` lists the twenty-nine steps the job runner runs, resumes and times, and
`StepName::stage` gives each its stage. `as_str` holds each name; `Display` and serde's
`snake_case` spell the same ones, and `FromStr` answers `UnknownStage` or `UnknownStep` with the
text it was given. Where each step runs is the pipeline's step graph, not this module.

## Boundaries

- Depends on: `serde` (`Serialize`, `Deserialize`), `rkyv` (the archived names) and `std`.
- Used by: `crates/job_model/src/lib.rs`, which re-exports `StageName` and `StepName`;
  `crates/pipeline/` (the step graph, the resume check, the workers, the job store and the
  report); the `worker` and `process` subcommands in `apps/tbd_subtitles/src/cli/` and the worker
  binaries in `apps/tbd_subtitles_ggml/` and `apps/tbd_subtitles_llm/`, which parse step names;
  the window's queue and report, which group steps by stage.
- Rules:
  - `ALL` lists every stage once, and each name parses back to its own stage
    (`every_stage_is_listed_once_and_parses_back_to_itself` in `tests/stage_name.rs`);
  - the JSON name equals the command-line name (`json_names_match_the_command_line_names`);
  - only the seven model and visual stages run in a worker (`only_model_stages_run_in_a_worker`);
  - the steps of a stage are contiguous and the stages follow `StageName::ALL`
    (`steps_follow_the_stage_order` in `tests/step_name.rs`);
  - every name round-trips through rkyv, and archived step names keep the run order
    (`every_step_name_round_trips`, `archived_step_names_keep_the_run_order` in
    `tests/archive.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — every step, where it
  runs and what it writes.
````

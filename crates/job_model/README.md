# Job model

The `job_model` crate: the serde types the pipeline's [stages](/documentation/glossary.md#stage)
write into a job's [work directory](/documentation/glossary.md#work-directory) and read back, which
are the contracts between them. It sits below every other product crate and depends on no
workspace crate. The stage names hold code; the job record, the stage outputs and the report are
not written yet.

## Contents

```text
crates/job_model/
├── Cargo.toml  the `job_model` library package: `serde` with derive, and `serde_json` for the tests
└── src/        the stage names, and the module folders for the job record, stage outputs and report
```

## How it works

`StageName` is the one list of stages. `StageName::ALL` holds the eleven stages in the order the
job runner runs them; `as_str` gives each the one name used on the command line, in file names and
in JSON, and `Display`, `FromStr` and serde's `snake_case` all use that name; `runs_in_worker`
says which stages load a GPU model or the language model and so run in a
[worker process](/documentation/glossary.md#worker-process). The app's `worker` subcommand parses
its stage argument through `FromStr` and refuses a stage for which `runs_in_worker` is false.

The `job`, `outputs` and `report` modules hold only their one-line headers: the job record kept in
`job.json`, one typed output per stage, and the job report. `src/README.md` describes each module.

## Getting started

Run these from the repository root:

```bash
cargo build -p job_model   # the library
cargo test -p job_model    # 5 unit tests for the stage names, well under a second
```

## Configuration

None: the crate reads no setting.

## Public surface

- The library `job_model`, with `StageName` re-exported at its root and the modules `stage`
  (`StageName`, `UnknownStage`), `job`, `outputs` and `report`; the last three hold no items yet.
- No binary.

## Boundaries

- Depends on: `serde` 1 with `std` and `derive`; `serde_json` 1 in the tests only. No workspace
  crate.
- Used by:
  - the app: `apps/tbd_subtitles/src/cli/mod.rs` parses the `worker` stage with `StageName`, and
    `apps/tbd_subtitles/src/cli/worker_command.rs` takes it;
  - `crates/media_io/`, `crates/inference/`, `crates/subtitle_formats/`, `crates/stages/` and
    `crates/pipeline/`, which declare it as a dependency and use nothing from it yet.
- Rules:
  - the crate sits in layer 0 and depends on no workspace crate (`cargo gates crate-layering`,
    layer table in `tools/repo_gates/src/layout.rs`);
  - each stage name parses back to its own stage and is the same in JSON and on the command line,
    so a resumed job reads what an earlier run wrote
    (`every_stage_is_listed_once_and_parses_back_to_itself` and
    `json_names_match_the_command_line_names` in `crates/job_model/src/stage/tests/stage_name.rs`);
  - a type here changes only together with every stage that reads or writes it (review).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the job
  work directory and the file each stage writes.
- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — the stages and their order.

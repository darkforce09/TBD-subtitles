# Stage names

Every [stage](/documentation/glossary.md#stage) of the pipeline, in run order, with the one name
used on the command line, in file names and in JSON, and whether it runs in a
[worker process](/documentation/glossary.md#worker-process).

## Contents

```text
crates/job_model/src/stage/
├── mod.rs         the module tree and the re-export of the stage name
├── stage_name.rs  every stage in run order, with its command-line and JSON name
└── tests/         unit tests for the stage names
```

## How it works

`StageName::ALL` lists the stages in the order the job runner runs them, which is not the order
the enum declares them in:

| # | Name | Runs in a worker |
|---|---|---|
| 1 | `probe_decode` | no |
| 2 | `separation` | yes |
| 3 | `vad` | no |
| 4 | `asr` | yes |
| 5 | `diff_sheet` | no |
| 6 | `sound_events` | yes |
| 7 | `adjudication` | yes |
| 8 | `alignment` | yes |
| 9 | `cues` | no |
| 10 | `qc` | no |
| 11 | `output` | no |

`as_str` holds the names; `Display` and serde's `snake_case` spell the same ones, and `FromStr`
searches `ALL` for a match, answering `UnknownStage` with the text it was given (its message reads
`` `subtitles` is not a stage ``). The match in `as_str` is exhaustive, so a new variant cannot
build without a name; its place in `ALL`, and the array's length, are set by hand.

## Boundaries

- Depends on: `serde` (`Serialize`, `Deserialize`) and `std`.
- Used by: `crates/job_model/src/lib.rs`, which re-exports `StageName`; the app's
  `apps/tbd_subtitles/src/cli/mod.rs`, which accepts a `worker` stage only when `runs_in_worker`
  is true, and `apps/tbd_subtitles/src/cli/worker_command.rs`.
- Rules:
  - `ALL` lists every stage once, and each name parses back to its own stage
    (`every_stage_is_listed_once_and_parses_back_to_itself` in `tests/stage_name.rs`); an unknown
    name is refused with that name (`an_unknown_name_is_refused_with_the_name`);
  - the JSON name equals the command-line name (`json_names_match_the_command_line_names`);
  - sound events run before adjudication, which chooses the sound cues, and adjudication before
    alignment (`sound_events_come_before_adjudication_which_chooses_the_cues`);
  - only the five model stages run in a worker (`only_model_stages_run_in_a_worker`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#stage-flow) — what each stage does.
- [Decisions](/documentation/decisions.md) — why each GPU stage runs in its own worker process.

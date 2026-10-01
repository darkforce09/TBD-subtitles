# Stage and step names

Every [stage](/documentation/glossary.md#stage) of the pipeline and every step the stages are made
of, in run order, with the one name each carries on the command line, in the job database's keys and in JSON.

## Contents

```text
crates/job_model/src/stage/
├── mod.rs         the module tree and the re-exports of the stage and step names
├── stage_name.rs  every stage in run order, with its name and whether it runs in a worker
├── step_name.rs   every step in run order, with its name and the stage it belongs to
└── tests/         unit tests for the stage and step names, JSON and rkyv
```

## How it works

`StageName::ALL` lists the thirteen stages in run order, which is not the order the enum declares
them in; `runs_in_worker` says which stages load a GPU model or the language model, or run their
steps in workers for the on-screen text and the localized video. A stage runs as one or more steps:
`StepName::ALL` lists the twenty-nine steps the job runner runs, resumes and times, and
`StepName::stage` gives each its stage.

| # | Stage | Steps |
|---|---|---|
| 1 | `probe_decode` | `probe_decode`, `shot_scan` |
| 2 | `separation` | `separation` |
| 3 | `vad` | `vad` |
| 4 | `asr` | `asr_parakeet`, `asr_whisper` |
| 5 | `diff_sheet` | `diff_sheet` |
| 6 | `sound_events` | `sound_events` |
| 7 | `adjudication` | `adjudicate`, `redecode_parakeet`, `redecode_whisper`, `readjudicate`, `sound_cues` |
| 8 | `alignment` | `alignment`, `review` |
| 9 | `cues` | `cues` |
| 10 | `onscreen_text` | `text_detect`, `text_read`, `text_track`, `text_translate`, `text_review`, `text_mask`, `text_inpaint`, `text_compose`, `text_verify`, `text_typeset` |
| 11 | `qc` | `qc` |
| 12 | `output` | `output` |
| 13 | `localized_video` | `localized_video` |

`as_str` holds each name; `Display` and serde's `snake_case` spell the same ones, and `FromStr`
searches `ALL` for a match, answering `UnknownStage` or `UnknownStep` with the text it was given
(`` `subtitles` is not a stage ``). The matches in `as_str` and `stage` are exhaustive, so a new
variant cannot build without a name and a stage; its place in `ALL` is set by hand. Where each
step runs is the pipeline's step graph, not this module.

## Boundaries

- Depends on: `serde` (`Serialize`, `Deserialize`), `rkyv` (the archived names) and `std`.
- Used by: `crates/job_model/src/lib.rs`, which re-exports `StageName` and `StepName`;
  `crates/job_model/src/job/`, whose record keys steps by `StepName`; `crates/pipeline/` (the step
  graph, the resume check, the workers, the progress and the work directory); the `worker` and
  `process` subcommands in `apps/tbd_subtitles/src/cli/` and the worker binaries in
  `apps/tbd_subtitles_ggml/src/main.rs` and `apps/tbd_subtitles_llm/src/main.rs`, which parse
  step names; the window's queue and report, which group steps by stage; and
  `crates/stages/src/qc/markdown.rs`.
- Rules:
  - `ALL` lists every stage once, and each name parses back to its own stage
    (`every_stage_is_listed_once_and_parses_back_to_itself` in `tests/stage_name.rs`); an unknown
    name is refused with that name (`an_unknown_name_is_refused_with_the_name`);
  - the JSON name equals the command-line name (`json_names_match_the_command_line_names`);
  - sound events run before adjudication, which chooses the sound cues, and adjudication before
    alignment (`sound_events_come_before_adjudication_which_chooses_the_cues`);
  - only the seven model and visual stages run in a worker (`only_model_stages_run_in_a_worker`);
  - every step is listed once and parses back, and serialises by its name
    (`every_step_is_listed_once_and_parses_back`, `steps_serialise_by_name` in
    `tests/step_name.rs`);
  - the steps of a stage are contiguous and the stages follow `StageName::ALL`
    (`steps_follow_the_stage_order`);
  - every name round-trips through rkyv, and archived step names keep the run order
    (`every_stage_name_round_trips`, `every_step_name_round_trips`,
    `archived_step_names_keep_the_run_order` in `tests/archive.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#steps-and-processes) — every step, where it
  runs and what it writes.
- [Decisions](/documentation/decisions/) — why each GPU stage runs in its own worker process.

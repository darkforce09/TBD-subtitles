# Job model source

The `job_model` library: one module folder per kind of contract that stages write into a job's
work directory and read back. Only the stage names hold code; the other modules are not written
yet.

## Contents

```text
crates/job_model/src/
├── job/      the job record kept in `job.json`: video, probe result, settings, stage status and times
├── lib.rs    the crate root: the module list and the `StageName` re-export
├── outputs/  the typed output of each stage, one JSON file per stage in the work directory
├── report/   the job report: quality-check results, flagged lines, stage times and peak memory
└── stage/    every stage in run order, with its command-line and JSON name
```

## How it works

`stage/` names the stages; the job runner, the app's `worker` subcommand and the work directory all
use those names. `job/` is for the record of one job, `outputs/` for what each stage writes, and
`report/` for the job report. `outputs/` holds the probe result, the shot changes, the speech plan and the transcripts; `job/` and
`report/` hold only a `mod.rs` with its one-line header. Everything here
is plain data: no module reads a file, spawns a process or holds state.

## Public surface

- `StageName`, re-exported at the crate root from `stage`: used by `apps/tbd_subtitles/src/cli/`.
- `stage::UnknownStage`: the error of parsing a name that names no stage.
- `outputs`: `ProbeResult`, `VideoStream`, `AudioStream`, `ShotChanges`, `ShotCut`,
  `SpeechPlan`, `TimeSpan`, `EngineTranscript`, `ChunkWords` and `TimedWord`, for
  `crates/media_io/`, `crates/inference/`, `crates/stages/` and the stack spike tools.
- `job` and `report`: public modules with no items yet.

## Boundaries

- Depends on: `serde` for the derives in `stage/` and `outputs/`.
- Used by: the app's command line in `apps/tbd_subtitles/src/cli/`, and every other product crate
  through its manifest.
- Rules: the JSON names never change once a stage writes them, so a resumed job reads what an
  earlier run wrote (`json_names_match_the_command_line_names` in `stage/tests/stage_name.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the files
  of a job's work directory.

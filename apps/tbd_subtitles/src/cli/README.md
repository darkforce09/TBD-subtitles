# Command line

The subcommands of the `tbd-subtitles` binary: `gui` opens the window, `process` runs videos
without one, and `worker` runs one GPU [stage](/documentation/glossary.md#stage) of a job in its
own process. People run the first two; `worker` is the entry point a job runner starts.

## Contents

```text
apps/tbd_subtitles/src/cli/
├── mod.rs              `Cli` and its subcommands in clap, the dispatch, and the worker-stage check
├── process_command.rs  the `process` runner: checks every video is a readable file; no stage runs
├── tests/              parsing, dispatch and refusal tests of the three subcommands
└── worker_command.rs   the `worker` runner: one stage over one job's work directory; no stage runs
```

## How it works

`run` parses the process arguments with clap's derive API and `dispatch` sends each subcommand to
its runner; with no subcommand it opens the window, as a desktop launcher expects. Every runner
returns an `anyhow::Result`, which `apps/tbd_subtitles/src/main.rs` turns into the exit code.
clap itself prints the usage and exits 2 on a usage error, including a stage refused by
`parse_worker_stage`.

```text
tbd-subtitles [COMMAND] ──▶ Cli::parse ──▶ dispatch
   (none) | gui [VIDEOS]...          ──▶ crate::application::launch(videos)
   process <VIDEOS>...               ──▶ process_command::run
   worker <STAGE> <JOB_DIR>          ──▶ worker_command::run   (STAGE checked by parse_worker_stage)
```

No pipeline stage is built, so `process` and `worker` end with an error that names what they
could not do; neither reports success.

## Commands

Each runs as `cargo run -p tbd_subtitles -- <arguments>` from the repository root, or as
`tbd-subtitles <arguments>` once built. `--help` on the binary or a subcommand prints its usage;
`--version` prints the version.

### gui

- Synopsis: `tbd-subtitles gui [VIDEOS]...`, or `tbd-subtitles` with no subcommand and no videos.
- Does: opens the desktop window with the given videos in the queue, and returns when the window
  closes.
- Exit codes: 0 when the window closes; 1 when it cannot open; 2 on a usage error.
- Example: `cargo run -p tbd_subtitles -- gui episode_01.mkv episode_02.mkv`

### process

- Synopsis: `tbd-subtitles process <VIDEOS>...`, at least one video.
- Does: checks that every video is a readable file, naming the first that is not, then stops with
  an error that no pipeline stage is built and so no subtitles can be generated.
- Exit codes: 1 for a missing or unreadable video, a path that is not a file, or the missing
  stages; 2 when no video is given.
- Example: `cargo run -p tbd_subtitles -- process episode_01.mkv`

### worker

- Synopsis: `tbd-subtitles worker <STAGE> <JOB_DIR>`.
- Does: accepts only the stages that run in a
  [worker process](/documentation/glossary.md#worker-process) (`separation`, `asr`,
  `sound_events`, `adjudication`, `alignment`); a stage that runs inside the job runner is refused
  with the list of worker stages. It then stops with an error that the stage is not built.
- Exit codes: 1 for an accepted stage, which is not built; 2 for an unknown stage, a stage that
  does not run in a worker, or a missing argument.
- Example: `cargo run -p tbd_subtitles -- worker asr work/job_dir`

## Boundaries

- Depends on: `crate::application::launch`; `job_model::StageName` from `crates/job_model/`, with
  its `runs_in_worker`, `ALL` and `as_str`; `clap` and `anyhow`.
- Used by: `apps/tbd_subtitles/src/main.rs`, which calls `run`.
- Rules:
  - running with no subcommand opens the window (`no_subcommand_opens_the_window`), and the clap
    definition stays consistent (`the_command_definition_is_consistent`);
  - `gui` takes optional videos and `process` needs at least one
    (`gui_takes_optional_videos`, `process_needs_at_least_one_video`);
  - `worker` accepts GPU stages only and refuses the rest by name
    (`worker_accepts_gpu_stages_only`);
  - `process` names a missing video, and neither `process` nor `worker` reports success while the
    stages are missing (`process_refuses_a_missing_video_by_name`,
    `process_and_worker_never_report_success_before_the_stages_exist`);
  - all in `tests/cli.rs`.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the `gui`, `process` and
  `worker` processes.
- [Automation](/documentation/features/automation.md) — the Dolphin entry and watch folders
  that run `process`.

**Status:** live

# README template: command-line

**When to use:** a folder that defines a binary's subcommands, such as the app's
`apps/tbd_subtitles/src/cli/`. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the command-line kind adds Commands.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. Commands holds
one `###` heading per command the folder defines, with the same four bullets under each.

````markdown
# <Name of the command line, in plain words: no path, no backticks>

<One to three sentences: what the commands are for and who runs them.>

## Contents

```text
<repository path of the folder>/
├── <file>  <the command it defines, or the parsing it holds: a lowercase phrase, no closing period>
└── tests/  <what the tests cover>
```

## How it works

<How an invocation flows: where the arguments are parsed, where each command's work runs, and how
the exit code is formed. An ASCII diagram in a text block helps here.>

## Commands

<One line on how to run the commands, and what they share: the usage error, the error report.>

### <command name>

- Synopsis: `<the usage line, as the command's own --help prints it>`
- Does: <what it does>
- Exit codes: <each code and what it means, as the code returns them>
- Example: `<one real invocation, run from the repository root>`

## Boundaries

- Depends on: <the modules and crates the commands call, read from their imports>
- Used by: <the people, launchers and processes that run the commands, found with git grep>
- Rules: <the invariants a change here must keep: stable names, where parsing lives, and the test
  or gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

Written from `apps/tbd_subtitles/src/cli/`, its clap definitions and its tests, and shortened: the
synopses leave out most options, which the folder's own README.md lists in full. The sample sits
in a fenced block, so no gate reads it as a README; the folder's own README.md is written from the
same code and may differ.

````markdown
# Command line

The subcommands of the `tbd-subtitles` binary: `gui` (or videos with no subcommand) opens the
window, `process` runs a job for each video without one, `fix` runs
[Fix It](/documentation/glossary.md#fix-it) on a finished video, `dump` prints the rows of a job's
database as JSON, and `worker` runs one step of a job in its own
[worker process](/documentation/glossary.md#worker-process). People run the first four; `worker`
is the job runner's to start.

## Contents

```text
apps/tbd_subtitles/src/cli/
├── dump_command.rs     the `dump` job resolution, the job database opened if free, its rows as JSON
├── fix_command.rs      the `fix` options over the settings file, Fix It, the correction run
├── mod.rs              `Cli` and its subcommands in clap, and the dispatch to each runner
├── process_command.rs  the `process` options over the settings file, folders expanded, the run
├── tests/              parsing, the window's starts, settings, exit codes, the worker step check, dump
├── window_command.rs   videos alone, `gui` and `process --enqueue`: the single instance or a hand-off
└── worker_command.rs   the `worker` runner and its step check: every step but the Whisper ones
```

## How it works

`run` parses the arguments with clap's derive API. The starts that concern the window (videos
with no subcommand, `gui`, `process --enqueue`) go to `window_command`, which claims the single
instance and opens the window, or hands the videos to the window already open. Every other
subcommand installs logging to stderr and `dispatch` sends it to its runner. Every runner returns
an `anyhow::Result` of the exit code, which `main` returns, or 1 with the error chain on stderr
after `tbd-subtitles:`; clap prints the usage and exits 2 on a usage error.

```text
tbd-subtitles [COMMAND] ──▶ Cli::parse
   [VIDEOS]... | gui | process --enqueue ──▶ window_command::run ──▶ launch, or hand off
   process <PATHS>...                    ──▶ process_command::run ──▶ pipeline::run_job, per video
   fix <VIDEO>                           ──▶ fix_command::run     ──▶ pipeline::fix_it::fix_video
   dump <JOB> <TABLE> [KEY]              ──▶ dump_command::run    ──▶ JobStore::open_existing
   worker <STEP> <JOB_DIR>               ──▶ worker_command::run  ──▶ pipeline::tasks::worker_main
```

## Commands

Each runs as `cargo run -p tbd_subtitles -- <arguments>` from the repository root, or as
`tbd-subtitles <arguments>` once built; `--help` prints the usage.

### gui

- Synopsis: `tbd-subtitles gui [VIDEOS]...`, or `tbd-subtitles [VIDEOS]...` with no subcommand.
- Does: opens the window with the videos added to its queue and returns when it closes; when the
  window is open already, hands it the videos and returns at once.
- Exit codes: 0 the window closed, or the open window took the videos; 1 it cannot open, or the
  open window did not answer within 5 s; 2 usage.
- Example: `cargo run -p tbd_subtitles -- episode_01.mkv episode_02.mkv`

### process

- Synopsis: `tbd-subtitles process <PATHS>... [--settings <FILE>] [--format <srt|vtt|ass>] [--rerun <STEP>]...`,
  or `tbd-subtitles process --enqueue <PATHS>...`
- Does: runs one job per video, in order, a folder giving every video under it without a subtitle
  file, and writes the subtitles beside each video; each job resumes from the steps whose output is
  still valid. With `--enqueue` it queues the videos in the window instead.
- Exit codes: 0 every job passed the quality check; 2 one failed it, named on the last line; 1 a
  missing video, an invalid settings file, a missing model or a failed step; 2 also on a usage
  error.
- Example: `distrobox-host-exec target/release/tbd-subtitles process "Dressrosa 08.mp4" --rerun cues`

### fix

- Synopsis: `tbd-subtitles fix <VIDEO> [--settings <FILE>] [--model <MODEL>] [--processes <N>]`
- Does: runs Fix It on the video's finished job (`opus` unless told otherwise), then runs the job
  again when it changed a line, so the changes reach the subtitle file.
- Exit codes: 0 done, changed or not; 1 a missing video, an unfinished job or a failed correction
  run; 2 usage.
- Example: `distrobox-host-exec target/release/tbd-subtitles fix "[Muhn Pace] Dressrosa 12.mp4"`

### dump

- Synopsis: `tbd-subtitles dump <JOB_OR_VIDEO> <TABLE> [KEY] [--settings <FILE>] [--work-root <DIR>]`
- Does: opens the job's `job.redb` only when no other process owns it and prints one row of
  `<TABLE>` (`meta`, `step_records`, `outputs`, `corrections`, `frames` or `readings`) as pretty
  JSON, or every row as JSON Lines.
- Exit codes: 0 printed; 1 no such job or row, or a job another process is running; 2 usage.
- Example: `distrobox-host-exec target/release/tbd-subtitles dump "[Muhn Pace] Dressrosa 11.mp4" meta layout`

### worker

- Synopsis: `tbd-subtitles worker <STEP> <JOB_DIR>`
- Does: runs one step of the job in this process, exchanging frames of the worker channel with the
  job runner; the Whisper steps are refused as belonging to `tbd-subtitles-ggml`.
- Exit codes: 0 the step finished; 1 the job cannot be loaded or the step failed; 2 usage.
- Example: `target/release/tbd-subtitles worker separation ~/.local/share/tbd-subtitles/work/<job>`

## Boundaries

- Depends on: `crate::application` and `crate::core::single_instance` for the window; `pipeline`
  (`run_job`, `fix_it`, `tasks::worker_main`, `work_dir::JobStore`); `worker_channel::address`;
  `job_model::StepName`; `crate::settings`; `clap`, `serde_json` and `anyhow`.
- Used by: `apps/tbd_subtitles/src/main.rs`, which calls `run`; the job runner in
  `crates/pipeline/`, which starts `worker`; Dolphin's entry, which runs `process --enqueue`.
- Rules:
  - running with no subcommand opens the window (`no_subcommand_opens_the_window` in
    `tests/cli.rs`), and only the window's starts claim the single instance
    (`gui_and_enqueue_concern_the_window_and_the_rest_do_not`);
  - an option wins over the settings file (`options_win_over_the_settings_file`), and the exit
    code says whether every job passed the quality check
    (`the_exit_code_says_whether_every_job_passed_the_quality_check`);
  - `worker` refuses the Whisper steps (`worker_takes_main_binary_steps_only`);
  - `dump` finds a job by its folder, its name or its video
    (`a_job_is_found_by_its_folder_its_name_or_its_video` in `tests/dump_command.rs`);
  - the step names are an interface the job runner calls by name, and `StepName` owns them.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — the steps a `process` run goes through.
- [Automation](/documentation/features/automation.md) — the Dolphin entry that runs
  `process --enqueue`, and the watch folders.
````

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

Written from `apps/tbd_subtitles/src/cli/`, the binary's own `--help` output and runs of each
subcommand. The sample sits in a fenced block, so no gate reads it as a README; the folder's own
README.md is written from the same code and may differ.

````markdown
# Command line

The subcommands of the `tbd-subtitles` binary: `gui` opens the desktop window, `process` runs jobs
without one, and `worker` runs one GPU [stage](/documentation/glossary.md#stage) of a job in its
own process. People run the first two; `worker` is the job runner's to start.

## Contents

```text
apps/tbd_subtitles/src/cli/
├── mod.rs              the clap definition of the three subcommands, the dispatch, the worker stage check
├── process_command.rs  the `process` subcommand: checks each video, then runs its job
├── tests/              parsing, the worker stage check, and the refusal to report false success
└── worker_command.rs   the `worker` subcommand: one stage of one job
```

## How it works

`main` calls `cli::run`, which parses the arguments with clap's derive API and dispatches: no
subcommand and `gui` call `application::launch`, `process` calls `process_command::run`, and
`worker` calls `worker_command::run`. The stage argument of `worker` is parsed by
`parse_worker_stage`, which accepts a stage name only when that stage runs in a
[worker process](/documentation/glossary.md#worker-process). A runner returns an `anyhow` error
chain, which `main` prints after `tbd-subtitles:` on stderr before exiting 1; clap prints its own
usage errors and exits 2.

```text
main ──▶ cli::run ──▶ (none) / gui ──▶ application::launch
                  ├─▶ process      ──▶ process_command::run
                  └─▶ worker       ──▶ worker_command::run
```

## Commands

Each runs as `cargo run -p tbd_subtitles -- <arguments>`, or as `tbd-subtitles <arguments>` once
built. `--help` on the binary or on a subcommand prints its usage.

### gui

- Synopsis: `tbd-subtitles gui [VIDEOS]...`; running with no subcommand does the same with an empty
  queue.
- Does: opens the window with the named videos in the queue, and returns when it closes.
- Exit codes: 0 the window closed; 1 the window could not open; 2 usage.
- Example: `cargo run -p tbd_subtitles -- gui a.mkv b.mkv`

### process

- Synopsis: `tbd-subtitles process <VIDEOS>...`
- Does: checks that each video is a readable file, then runs one job per video, in order. No
  pipeline stage is built, so after the check it stops with
  `no pipeline stage is built yet, so no subtitles can be generated for <n> video(s)`.
- Exit codes: 1 a video is missing or not a file, or its job cannot run, which is every job
  while no stage is built; 2 usage, including no video named.
- Example: `cargo run -p tbd_subtitles -- process "Dressrosa 08.mp4"`

### worker

- Synopsis: `tbd-subtitles worker <STAGE> <JOB_DIR>`
- Does: runs one stage of the job in `<JOB_DIR>` in this process. `<STAGE>` is one of
  `separation`, `asr`, `sound_events`, `adjudication` or `alignment`; any other stage name is
  refused with the list. No stage is built, so it stops with
  `the <stage> stage is not built yet; nothing was run for <JOB_DIR>`.
- Exit codes: 1 the stage cannot run, which is every stage while none is built; 2 usage,
  including a stage that runs inside the job runner or a name that is no stage.
- Example: `cargo run -p tbd_subtitles -- worker asr /tmp/job`

## Boundaries

- Depends on: `clap` for parsing; `job_model::StageName` for the worker stage names; the
  `application` module for the window; `anyhow` for the error chain.
- Used by: `main.rs`, and people at a terminal; nothing in the repository starts `worker` yet.
- Rules:
  - running with no subcommand opens the window, as a desktop launcher expects
    (`no_subcommand_opens_the_window` in `tests/cli.rs`);
  - `worker` accepts only the stages that run in a worker process
    (`worker_accepts_gpu_stages_only`);
  - a command that cannot do its work exits non-zero and never reports success
    (`process_and_worker_never_report_success_before_the_stages_exist`);
  - the subcommand and stage names are an interface other programs call by name, so they stay
    stable; `StageName` owns the stage names.

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the `gui`, `process` and
  `worker` processes.
- [Automation](/documentation/features/automation.md) — the launchers that run `process`.
````

**Status:** live

# README template: crate root

**When to use:** the folder that holds a crate's `Cargo.toml`: the app in `apps/tbd_subtitles/`,
each library crate under `crates/`, each tool under `tools/`. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the crate root kind adds Getting started, Configuration and Public surface.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. A kind section
with nothing to hold keeps its heading and says so in one line.

````markdown
# <Crate name, in plain words: no path, no backticks>

<One to three sentences: what the crate is, with its package name, and who uses it.>

## Contents

```text
<repository path of the folder>/
├── Cargo.toml  <the package: name, targets, and what each dependency is for>
└── src/        <what the source tree holds, in one phrase>
```

## How it works

<What happens from the entry point to the result: the main types, the layers of the source tree,
and the invariants that span them. Name each child's part in one clause.>

## Getting started

<The commands, run from the repository root, that build, run and test the crate, each with what
to expect; say which stay in the foreground. Link the runbook for the full procedure.>

## Configuration

<Every setting the crate reads: environment variables, files and Cargo features, each with its
default, whether it is required, and the file that reads it; or one line saying it reads none.>

## Public surface

- <module, type, function or binary>: <what it offers and who outside the crate uses it>

## Boundaries

- Depends on: <the workspace crates, crates.io crates and programs it uses, from its Cargo.toml
  and its calls>
- Used by: <every crate, tool or person that uses it, found with git grep>
- Rules: <the invariants a change here must keep, and the test or gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

Written from `crates/child_process/`, its `Cargo.toml`, its source and its tests, and shortened:
the folder's own README.md lists every user and every rule. The sample sits in a fenced block, so
no gate reads it as a README; the folder's own README.md is written from the same code and may
differ.

````markdown
# Child processes

The `child_process` crate: the one way the app and the repository tools start an external program.
Every child runs in its own process group under an optional deadline, with both pipes drained for
its whole life, dies with the thread that started it, and its result tells a raw exit code apart
from a signal, a timeout and a missing program.

## Contents

```text
crates/child_process/
├── Cargo.toml  the `child_process` library package; its dependencies are `libc` and `tracing`
└── src/        the `Run` builder, the process-group runner, the pipe drains and the lookup helpers
```

## How it works

A caller builds a `Run` with `Run::new(program)` and the builder methods `arg`, `args`, `cwd`,
`env`, `env_remove`, `timeout`, `cancel_on`, and `stdin` or `stdin_piped`, then finishes it with
`output` (stdout and stderr apart), `merged_output` (both on one shared pipe, as a shell's `2>&1`),
`status` (the raw exit code alone) or `spawn`, which hands stdout to the caller as a stream in a
`Running` child whose `wait` gives a `Finished`.

Every call either returns the child's real exit code or a `RunError` saying why there is none:
`ProgramAbsent`, `Failed`, `Signalled` (never turned into a `128+n` code), `Timeout` or
`Cancelled`. Between fork and exec, `setsid` puts the child in a process group of its own and
`prctl(PR_SET_PDEATHSIG, SIGKILL)` ties its life to the thread that started it; `killpg` kills the
whole group at the deadline, so a forking program such as FFmpeg or `cargo` leaves nothing behind.
Every child is also logged as `tracing` events under the `child_process` target: its start, each
stderr line and its end, never its stdout.

## Getting started

Run these from the repository root:

```bash
cargo build -p child_process   # the library alone
cargo test -p child_process    # the unit tests; they run sh, cat, sleep and seq, about 3 s
```

## Configuration

The crate reads one setting, the `PATH` environment variable, in `which` (`src/lookup.rs`); an
unset `PATH` answers `ProgramAbsent`. A child inherits this process's environment plus the `env`
pairs and minus the `env_remove` names of its `Run`. Nothing else is read.

## Public surface

- The library `child_process`: `Run`, `Output`, `Merged` and `RunError`, the streamed child
  `Running` with its result `Finished`, and the functions `which`, `retry` and `wait_for`, all at
  the crate root. The runner and the pipe drains are private.
- No binary.

## Boundaries

- Depends on: `std`, `libc` 0.2 without default features, and the `tracing` facade; no workspace
  crate. Linux only, through `std::os::unix` process extensions and `prctl`.
- Used by: `tools/verification_core/`, for `git` and `cargo` in the repository gates;
  `crates/media_io/` (ffprobe and FFmpeg), `crates/inference/` (the `claude` CLI) and
  `crates/pipeline/` (the app's worker processes); the app in `apps/tbd_subtitles/`; and the
  tools `stack_spike`, `appimage_builder` and `visual_validation`.
- Rules:
  - the crate sits in layer 0 and depends on no workspace crate (`cargo gates crate-layering`);
  - a signal is never an exit code (`signal_death_is_signalled_not_an_exit_code` in
    `src/tests/runner.rs`), and a raw exit code passes through unchanged
    (`captures_stdout_and_raw_code`);
  - a timeout kills the whole process group (`timeout_kills_the_whole_process_group`), and a full
    pipe never deadlocks a run (`large_output_does_not_deadlock`);
  - a streamed child is killed at its deadline even while its reader blocks
    (`a_deadline_kills_a_reader_blocked_child` in `src/tests/running.rs`), and dies with the
    thread that started it (`a_child_dies_with_the_thread_that_started_it`).

## Related documentation

- [Coding standards](/documentation/standards/coding_standards.md#errors-and-processes) — every
  child process has a timeout and a drained stderr.
- [Decisions](/documentation/decisions/) — the external programs the app and the repository tools
  may start.
````

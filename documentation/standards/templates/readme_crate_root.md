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

Written from `crates/child_process/`, its tests and a `cargo test -p child_process` run. The
sample sits in a fenced block, so no gate reads it as a README; the folder's own README.md is
written from the same code and may differ.

````markdown
# Child processes

The `child_process` crate: the one way the app and the repository tools start an external
program. Every run can carry a deadline, runs in its own process group, and has its pipes drained
for its whole life, so a run always ends with an exit code or a stated reason why there is none.

## Contents

```text
crates/child_process/
├── Cargo.toml  the `child_process` library package; its only dependency is `libc`
└── src/        the `Run` builder, spawning and reaping, pipe draining, `PATH` lookup and waits
```

## How it works

A caller builds a `Run` (program, arguments, working directory, environment, stdin, deadline) and
ends it with `output`, which captures stdout and stderr apart, `merged_output`, which reads both
from one shared pipe in the order the child wrote them, or `status`, which keeps only the exit
code. `src/runner.rs` spawns the child through `setsid`, so it leads its own process group, and a
missed deadline kills the whole group with `killpg`. `src/stream.rs` hands each pipe to its own
thread from spawn to exit, so a child that fills one pipe never deadlocks the parent.
`src/lookup.rs` holds `which`, `retry` and `wait_for`.

A run that produced no exit code is a `RunError`, never a number: `ProgramAbsent` (not on the
`PATH`), `Failed` (spawning or waiting failed), `Signalled` (killed by a signal) or `Timeout` (the
deadline passed and the group was killed). An exit code passes through as the child returned it.

## Getting started

Run from the repository root:

```bash
cargo test -p child_process   # 20 unit tests; they start sh, cat and sleep, and take about 3 s
```

## Configuration

`which` reads the `PATH` environment variable. The crate reads no file and has no Cargo feature;
a child inherits the parent's environment apart from what `env` and `env_remove` change.

## Public surface

- `Run`: the builder (`arg`, `args`, `cwd`, `env`, `env_remove`, `stdin`, `timeout`) and its three
  endings (`output`, `merged_output`, `status`), plus `display` for diagnostics.
- `Output` and `Merged`: what a finished run produced, on two pipes or on one.
- `RunError`: why a run produced no exit code.
- `which`, `retry` and `wait_for`: find a program on the `PATH`, retry a run with a fixed backoff
  (never retrying an absent program), and poll a condition until a deadline.

## Boundaries

- Depends on: `std` and `libc` 0.2, for `setsid`, `setpgid` and `killpg`; no workspace crate.
- Used by: `tools/verification_core/`, whose `src/proc.rs` re-exports `Run`, `Output`, `Merged`
  and `RunError` and calls `which` to run `git` and `cargo` for the gates. `media_io`,
  `inference` and `pipeline` declare it in their `Cargo.toml` and call nothing yet.
- Rules:
  - the crate sits on the bottom layer and depends on no workspace crate
    (`cargo gates crate-layering`);
  - a signal is never an exit code (`signal_death_is_signalled_not_an_exit_code` in
    `src/tests/runner.rs`);
  - a timeout leaves no process of the group alive (`timeout_kills_the_whole_process_group`);
  - a full pipe never deadlocks a run (`large_output_does_not_deadlock`,
    `merged_output_times_out_without_deadlocking_on_a_full_pipe`);
  - an absent program is never retried (`retry_does_not_retry_an_absent_program`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the external programs the
  app starts and why FFmpeg's stderr is drained on its own thread.
- [Repository tooling may run git and cargo](/documentation/decisions.md#2026-09-25--repository-tooling-may-run-git-and-cargo)
  — which programs the app and the tools may start.
````

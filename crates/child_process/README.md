# Child processes

The `child_process` crate: the one way the app and the repository tools start an external program.
Every child runs in its own process group under an optional deadline, with both pipes drained for
its whole life, dies with the thread that started it, and its result tells a raw exit code apart
from a signal, a timeout and a missing program.

## Contents

```text
crates/child_process/
├── Cargo.toml  the `child_process` library package; its one dependency is `libc`
└── src/        the `Run` builder, the process-group runner, the pipe drains and the lookup helpers
```

## How it works

A caller builds a `Run` with `Run::new(program)` and the builder methods `arg`, `args`, `cwd`,
`env`, `env_remove`, `timeout` and `stdin`, then finishes it with one of four calls:

| Call | Pipes | Answer |
|---|---|---|
| `output` | stdout and stderr on two pipes | `Output`: `code`, `stdout`, `stderr`, `duration` |
| `merged_output` | both streams on one shared pipe, as a shell's `2>&1` | `Merged`: `code`, `text`, `duration` |
| `status` | as `output` | the raw exit code alone |
| `spawn` | stdout handed to the caller as a stream, stderr drained on a thread | `Running`: `pid`, `take_stdout`, `kill`, `wait` → `Finished`: `code`, `stderr`, `duration` |

Every call either returns the child's real exit code, never folded to 0 or 1, or a `RunError`
saying why there is none: `ProgramAbsent` (the program is on no `PATH` entry), `Failed` (spawning,
waiting or piping broke), `Signalled` (the child died on a signal, which is never turned into a
`128+n` code) or `Timeout` (the deadline passed and the child's process group was killed). A
`Running` child has a watchdog thread that kills its group at the deadline even while the caller
is blocked reading its stdout, and a handle dropped without `wait` kills its group too, so an
abandoned FFmpeg stream never keeps running.

`libc` supplies the process calls. Between fork and exec, `setsid`, with `setpgid(0, 0)` as the
fallback, puts the child in a process group of its own, and `prctl(PR_SET_PDEATHSIG, SIGKILL)` asks
the kernel to kill the child when the thread that started it ends; a child whose parent is already
gone (`getppid` differs) gives up before exec. `killpg` with `SIGKILL` kills the whole group when
the deadline passes, so a forking program such as FFmpeg or `cargo` leaves nothing behind, and a
killed app leaves no GPU worker holding memory. A caller therefore starts a child only from a
thread that lives until the child is reaped. The lookup helpers `which`, `retry` and `wait_for` apply the same rule: an answer never obtained is a
`RunError`, never a quiet success. `src/README.md` describes each file.

## Getting started

Run these from the repository root:

```bash
cargo build -p child_process   # the library alone
cargo test -p child_process    # 25 unit tests; they run sh, cat, sleep and seq, about 3 s
```

The tests need a Unix shell on the `PATH`. The group-kill test sleeps 2.5 s after its timeout to
prove that the grandchild it started never wrote its marker file.

## Configuration

The crate reads one setting, the `PATH` environment variable, in `which` (`src/lookup.rs`); an
unset `PATH` answers `ProgramAbsent`. A child inherits this process's environment plus the `env`
pairs and minus the `env_remove` names of its `Run`. Nothing else is read.

## Public surface

- The library `child_process`: `Run`, `Output`, `Merged` and `RunError`, the streamed child
  `Running` with its result `Finished` (from `Run::spawn`), and the functions `which`, `retry`
  and `wait_for`, all at the crate root. The runner and the pipe drains are private.
- No binary.

## Boundaries

- Depends on: `std` and `libc` 0.2 without default features; no workspace crate. Linux only,
  through `std::os::unix` process extensions and `prctl`.
- Used by:
  - `tools/verification_core/`, whose `tools/verification_core/src/proc.rs` re-exports `Run`,
    `Output`, `Merged` and `RunError` and calls `which` for the repository gates;
  - `crates/media_io/`: ffprobe and the shot scan through `Run`, the FFmpeg PCM stream through
    `Run::spawn`, and `RunError` in its error type;
  - `crates/inference/`: the `claude` CLI backend in `crates/inference/src/llm/claude_cli/mod.rs`;
  - `crates/pipeline/`: the app's worker processes in `crates/pipeline/src/workers/mod.rs`;
  - `tools/stack_spike/`: its own measured workers in `tools/stack_spike/src/measure/mod.rs`.
- Rules:
  - the crate sits in layer 0 and depends on no workspace crate, and the repository tools may
    depend on it (`cargo gates crate-layering`, layer and tool tables in
    `tools/repo_gates/src/layout.rs`);
  - a signal is never an exit code (`signal_death_is_signalled_not_an_exit_code` in
    `crates/child_process/src/tests/runner.rs`), and a raw exit code passes through unchanged
    (`captures_stdout_and_raw_code`, same file);
  - a timeout kills the whole process group (`timeout_kills_the_whole_process_group`), and a full
    pipe never deadlocks a run (`large_output_does_not_deadlock`,
    `merged_output_times_out_without_deadlocking_on_a_full_pipe`), and a streamed child is killed
    at its deadline even while its reader blocks (`a_deadline_kills_a_reader_blocked_child` in
    `crates/child_process/src/tests/running.rs`) and when its handle is dropped unwaited
    (`dropping_an_unwaited_handle_kills_the_child`);
  - a child dies with the thread that started it
    (`a_child_dies_with_the_thread_that_started_it`, same file).

## Related documentation

- [Coding standards](/documentation/standards/coding_standards.md#errors-and-processes) — every
  child process has a timeout and a drained stderr.
- [Decisions](/documentation/decisions.md) — FFmpeg, ffprobe and the `claude` CLI as the app's only
  external programs, and `git` and `cargo` for the repository tools.

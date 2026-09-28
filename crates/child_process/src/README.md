# Child process source

The `child_process` library: the `Run` builder and its result types, the runner that spawns a
child in a process group of its own and reaps it within a deadline, the threads that drain its
pipes, and the helpers that find a program, retry an operation and wait on a condition.

## Contents

```text
crates/child_process/src/
├── lib.rs      the crate root: `Run` and its builder, `Output`, `Merged`, `RunError`, the re-exports
├── lookup.rs   the helpers `which`, `retry` and `wait_for`: a PATH lookup, a retry, a deadline poll
├── runner.rs   the calls `output`, `merged_output` and `status`: spawn in a new session, feed, reap
├── running.rs  the call `spawn`: a streamed stdout, a watchdog deadline and cancel flag, kill on drop
├── stream.rs   the drain threads: one per pipe for separate capture, one for the shared pipe
├── tests/      unit tests for the four calls, the error variants, the lookup helpers and the log
└── trace.rs    `Tag`: each child's start, stderr lines and end as `tracing` events
```

## How it works

```text
Run::new(program).arg(..).cwd(..).env(..).timeout(..).stdin(..)
  ├─ output()         two pipes ──▶ SeparateDrains: a thread per pipe ──▶ Output
  ├─ merged_output()  one std::io::pipe for both ──▶ one drain thread ──▶ Merged
  ├─ status()         output(), then the code alone
  └─ spawn()          stdout to the caller, stderr drain thread, watchdog ──▶ Running ──wait──▶ Finished
        │
        ├─ command()      pre_exec: setsid (or setpgid(0, 0)), then PR_SET_PDEATHSIG=SIGKILL
        ├─ spawn()        NotFound ──▶ ProgramAbsent; any other error ──▶ Failed
        ├─ feed_stdin()   write the body once and close the pipe
        ├─ wait_within()  no deadline: wait; a deadline: try_wait every 20 ms,
        │                 then killpg(SIGKILL), reap ──▶ Timeout
        └─ signal check   status.signal() ──▶ Signalled; otherwise the raw code
```

- `lib.rs` holds the data: `Run` stores the program, its arguments, the working folder, the
  environment changes, the deadline and the stdin body. `Run::display` renders the program and its
  arguments joined by spaces; it is the `program` in `Failed`, `Signalled` and `Timeout`, while
  `ProgramAbsent` carries the bare program name.
- `runner.rs` builds the `Command`. The child asks the kernel for SIGKILL when the thread that
  started it dies (`PR_SET_PDEATHSIG`), and gives up before exec if that parent is already gone,
  so a caller starts a child only from a thread that lives until the child is reaped. Because
  `setsid` makes the child a group leader, its pid is its process-group id, which `wait_within`
  hands to `killpg`. Stdin is a pipe only when the run carries a body; otherwise it is `/dev/null`,
  so a child that reads stdin sees EOF instead of this process's terminal. A closed stdin, when the
  child exits early, is not an error.
- `stream.rs` starts the drains before the wait and reads each pipe to EOF, so a child that fills
  one 64 KiB pipe buffer never blocks while the parent waits on the other. After a timeout the group
  is dead, both pipes are at EOF, and the drains join at once; after any other wait error the runner
  returns without joining, since a live child may still hold the pipes. Text decodes as lossy UTF-8,
  and a panicked drain yields an empty string, so captured text never costs the exit status.
  Stderr, and the shared pipe, is read line by line so each line is logged as it arrives; the
  text handed back is still every byte, and stdout is never logged.
- `merged_output` passes both write ends of one pipe to the child and then drops its own copies;
  otherwise the reader would never see EOF. The interleaving is the child's own, never a join of
  two strings.
- `running.rs` hands the child's stdout to the caller (FFmpeg's PCM pipe) and drains stderr on a
  thread. With a deadline or a cancel flag (`cancel_on`), a watchdog thread polls every 50 ms and
  kills the group when the deadline passes or the flag is set, so a caller blocked on a read sees
  EOF; `wait` then reports `Timeout` or `Cancelled`. `wait` closes an unread
  stdout before reaping, and dropping a `Running` that was never waited on kills its group.
- `trace.rs` names each child `program[pid]` and logs under the `child_process` target: its start
  with its command line at debug (an argument over 160 bytes or on several lines, such as a
  prompt or a schema, stands as its size), each stderr line at debug (split at carriage returns,
  blank parts skipped), and its end: exit 0 at debug, any other code, a signal or a timeout as a
  warning, a cancel at debug, a start that failed at debug, a stream dropped unwaited at debug.
  With no subscriber installed (the repository tools) the events cost nothing; the app's window
  shows them in its log window.
- `lookup.rs`: `which` returns the first `PATH` entry holding a file of that name. `retry` makes at
  least one attempt, sleeps a fixed backoff between attempts, returns the last error when all fail,
  and never retries `ProgramAbsent`. `wait_for` returns `Ok` only when its condition holds;
  running out of time is `Timeout` with the label as the program.

## Public surface

- `Run`, with `new`, `arg`, `args`, `cwd`, `env`, `env_remove`, `timeout`, `stdin`, `display`,
  `output`, `merged_output`, `status` and `spawn`: the one way `media_io`, `inference`, `pipeline`
  and the tools start a program; `tools/verification_core/src/proc.rs` re-exports it for the
  repository gates.
- `Output`, `Merged` and `RunError`: the answers, re-exported by the same file; `media_io` wraps
  `RunError` in its own error.
- `Running` and `Finished`: the streamed child and its result, for the FFmpeg PCM stream in
  `crates/media_io/src/pcm_stream/mod.rs` and the worker processes in
  `crates/pipeline/src/workers/mod.rs` and `tools/stack_spike/src/measure/mod.rs`.
- `which`, `retry` and `wait_for`: re-exported from `lookup.rs` at the crate root; the gates call
  `which` through `tools/verification_core/src/proc.rs`.

## Boundaries

- Depends on: `std` (processes, `std::io::pipe`, threads and the `std::os::unix` process
  extensions) and `libc` for `setsid`, `setpgid`, `prctl`, `getpid`, `getppid`, `killpg` and
  `SIGKILL`. `tracing` for the events of `trace.rs`; `tracing-subscriber` in the tests only.
- Used by: `tools/verification_core/src/proc.rs`; `crates/media_io/` (ffprobe, the shot scan and
  the PCM stream), `crates/inference/src/llm/claude_cli/mod.rs`,
  `crates/pipeline/src/workers/mod.rs` and `tools/stack_spike/src/measure/mod.rs`.
- Rules:
  - a signal is `Signalled`, never an exit code (`signal_death_is_signalled_not_an_exit_code` and
    `merged_output_reports_absent_tools_and_signals_honestly` in `tests/runner.rs`);
  - exit codes pass through raw on both paths (`captures_stdout_and_raw_code`,
    `merged_output_keeps_the_raw_exit_code`);
  - a timeout kills the whole group and returns `Timeout` (`timeout_kills_the_whole_process_group`,
    `timeout_reports_timeout`);
  - a streamed child dies at its deadline, on its cancel flag and on drop
    (`a_deadline_kills_a_reader_blocked_child`, `a_cancel_flag_kills_a_reader_blocked_child`,
    `dropping_an_unwaited_handle_kills_the_child` in `tests/running.rs`), and every child dies with the thread that started it
    (`a_child_dies_with_the_thread_that_started_it`, same file);
  - no pipe deadlocks a run (`large_output_does_not_deadlock`,
    `merged_output_times_out_without_deadlocking_on_a_full_pipe`), and the shared pipe keeps the
    child's order (`merged_output_preserves_interleaving`);
  - an absent program is never retried (`retry_does_not_retry_an_absent_program`), and an exhausted
    wait is never a success (`wait_for_times_out_rather_than_reporting_success`).

## Related documentation

- [Coding standards](/documentation/standards/coding_standards.md#errors-and-processes) — timeouts,
  drained stderr and checks that never report a run that did not happen as a success.

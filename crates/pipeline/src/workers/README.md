# Worker processes

Starting a step as a [worker process](/documentation/glossary.md#worker-process) of one of the
app's two binaries, forwarding its progress, keeping its stderr, and turning what it reports into
the step's measure.

## Contents

```text
crates/pipeline/src/workers/
├── gpu_lock.rs  the machine-wide GPU lock: one GPU worker at a time across every app process
├── mod.rs       `Binaries`, `run_worker` and `parse_line`
└── tests/       unit tests for the progress lines, a missing binary and the GPU lock
```

## How it works

`Binaries::beside_current_exe` finds `tbd-subtitles` and `tbd-subtitles-ggml` in the running
binary's folder. `run_worker` removes the step's old `steps/<step>.worker.json`, then starts
`<binary> worker <step> <job dir>` through `child_process::Run::spawn` with the step's timeout
from `graph`, the environment the runner passes (the CUDA runtime for GPU steps) and the job's
cancel token, which the child's watchdog watches: a cancelled worker is killed with its process
group and the step fails as cancelled. A GPU step first takes `gpu_lock`, an exclusive `flock` on
`gpu.lock` in the app data folder, waiting (and saying so once) while another process of the app
holds it; the kernel drops the lock when its holder dies. It reads the device's memory first and starts a `measure::gpu_monitor::Monitor` on the
worker's pid. Each stdout line becomes a progress event: `progress <done> <total>` an advance,
anything else a message. Its stderr lines are logged as they arrive (`child_process`), and when
the worker ends its stderr goes to `logs/<step>.log` too; a non-zero
exit is an error quoting the last 12 lines. Otherwise the worker's measure file and the VRAM peaks
become the step's `StepMeasure`, with the device's free memory before the step and its growth
during it as notes.
The step's GPU lock, the free VRAM it starts with and the path of its log file are debug
`tracing` events.

## Boundaries

- Depends on: `child_process` (`Run`, `Running`), `libc` (`flock`), `tracing`, `crate::cancel`, `job_model`
  (`StepMeasure`, `WorkerMeasure`),
  `crate::graph`, `crate::measure::gpu_monitor`, `crate::progress` and `crate::work_dir`.
- Used by: `crate::runner` for every step placed in a worker;
  `apps/tbd_subtitles/src/cli/process_command.rs` for `Binaries`.
- Rules:
  - one GPU worker runs at a time on the machine, and a cancelled wait never takes the lock
    (`a_held_lock_waits_until_released`, `a_cancelled_wait_gives_up` in `tests/gpu_lock.rs`);
  - a missing binary fails with where to look (`a_missing_binary_fails_with_where_to_look` in
    `tests/workers.rs`);
  - only a line of exactly `progress`, two numbers and nothing else is an advance
    (`progress_lines_become_advances_and_anything_else_a_message`);
  - a worker that exits non-zero or leaves no measure file is a failed step, and an old measure
    file is never read as the new one's (the module header);
  - the worker runs to its end on the calling thread, which the child dies with
    (`crates/child_process/src/lib.rs`).

## Related documentation

- [Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process)
  — why models load in workers.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why there are two binaries.

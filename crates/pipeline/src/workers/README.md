# Worker processes

Starting a step as a [worker process](/documentation/glossary.md#worker-process) of one of the
app's three binaries, reading its frames, forwarding its progress, keeping its stderr, and turning
what it reports into the step's measure.

## Contents

```text
crates/pipeline/src/workers/
├── frames.rs    `read_frames`: a worker's stdout frames as progress events and its `WorkerReport`
├── gpu_lock.rs  the machine-wide GPU lock: one GPU worker at a time across every app process
├── mod.rs       `Binaries` and `run_worker`
└── tests/       unit tests for the worker frames, a missing binary and the GPU lock
```

## How it works

`Binaries::beside_current_exe` finds `tbd-subtitles`, `tbd-subtitles-ggml` and
`tbd-subtitles-llm` in the running binary's folder. `run_worker` starts
`<binary> worker <step> <job dir>` through `child_process::Run::spawn` with the step's timeout
from `graph`, the environment the runner passes (the CUDA runtime for GPU steps) and the job's
cancel token, which the child's watchdog watches: a cancelled worker is killed with its process
group and the step fails as cancelled. A GPU step first takes `gpu_lock`, an exclusive `flock` on
`gpu.lock` in the app data folder, waiting (and saying so once) while another process of the app
holds it; the kernel drops the lock when its holder dies. It reads the device's memory first and
starts a `measure::gpu_monitor::Monitor` on the worker's pid.

The worker's stdout carries only frames of the worker channel (`crates/worker_channel/`), and
`frames::read_frames` reads them as bytes, never as lines:

| Frame | What the runner does |
|---|---|
| `Progress` | `Progress::StepAdvanced` with the step's done and total units |
| `ModelCall` | `Progress::ModelCall` from the call's JSON; JSON that does not parse is a short message naming its size |
| `Measure` | kept as the worker's `WorkerMeasure`, read from its rkyv archive with a check |
| `Failed` | kept as the worker's own reason, bad UTF-8 replaced |
| `Done` | the worker's end; any frame after it breaks the protocol |
| `Input`, `Output` | a protocol error: only the runner sends inputs, and no step stores outputs yet |

A protocol error (also a measure that does not check, a stream cut inside a frame or an unknown
tag) kills the worker at once (`Running::kill_and_wait`), writes its stderr to `logs/<step>.log`
and fails the step with the error and the log's last 12 lines. Its stderr lines are logged as they
arrive (`child_process`), and when the worker ends its stderr goes to `logs/<step>.log` too. A
non-zero exit is an error quoting the last 12 lines, preceded by the worker's `Failed` message
when it sent one; an exit 0 without `Measure` or without `Done` is an error naming what is
missing. Otherwise the worker's measure and the VRAM peaks become the step's `StepMeasure`, with
the device's free memory before the step and its growth during it as notes. The step's GPU lock,
the free VRAM it starts with and the path of its log file are debug `tracing` events.

## Boundaries

- Depends on: `child_process` (`Run`, `Running`), `worker_channel` (the frame codec), `rkyv`,
  `serde_json`, `libc` (`flock`), `tracing`, `crate::cancel`, `job_model` (`StepMeasure`,
  `WorkerMeasure`, `ModelExchange`),
  `crate::graph`, `crate::measure::gpu_monitor`, `crate::progress` and `crate::work_dir`.
- Used by: `crate::runner` for every step placed in a worker;
  `apps/tbd_subtitles/src/cli/process_command.rs` for `Binaries`.
- Rules:
  - one GPU worker runs at a time on the machine, and a cancelled wait never takes the lock
    (`a_held_lock_waits_until_released`, `a_cancelled_wait_gives_up` in `tests/gpu_lock.rs`);
  - a missing binary fails with where to look (`a_missing_binary_fails_naming_it` in
    `tests/workers.rs`);
  - a progress frame becomes an advance and a model call frame a model call, and a model call
    that does not parse a short message, never its bytes (`a_progress_frame_becomes_an_advance`,
    `a_model_call_frame_becomes_a_model_call`, `an_unreadable_model_call_is_a_short_message` in
    `tests/frames.rs`);
  - a failure message that is not UTF-8 is kept (`a_failure_that_is_not_text_is_kept`);
  - a frame after `Done`, a stream cut inside a frame, an unknown tag, an output, an input or a
    measure that does not check breaks the protocol
    (`a_frame_after_the_end_breaks_the_protocol`, `a_stream_cut_inside_a_frame_breaks_the_protocol`,
    `an_unknown_tag_breaks_the_protocol`, `an_output_or_a_bad_measure_breaks_the_protocol`);
  - a worker that exits non-zero, breaks the protocol, or exits 0 without `Measure` and `Done` is
    a failed step with the end of its stderr in the error (the module header of `mod.rs`);
  - the worker runs to its end on the calling thread, which the child dies with
    (`crates/child_process/src/lib.rs`).

## Related documentation

- [Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process)
  — why models load in workers.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why there are worker binaries.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#worker-channel) — the
  worker channel's frames and where they lead.

# Worker processes

Starting a step as a [worker process](/documentation/glossary.md#worker-process) of one of the
app's two binaries, forwarding its progress, keeping its stderr, and turning what it reports into
the step's measure.

## Contents

```text
crates/pipeline/src/workers/
├── mod.rs  `Binaries`, `run_worker` and `parse_line`
└── tests/  unit tests for the progress lines and a missing binary
```

## How it works

`Binaries::beside_current_exe` finds `tbd-subtitles` and `tbd-subtitles-ggml` in the running
binary's folder. `run_worker` removes the step's old `steps/<step>.worker.json`, then starts
`<binary> worker <step> <job dir>` through `child_process::Run::spawn` with the step's timeout
from `graph` and the environment the runner passes (the CUDA runtime for GPU steps). For a GPU
step it reads the device's memory first and starts a `measure::gpu_monitor::Monitor` on the
worker's pid. Each stdout line becomes a progress event: `progress <done> <total>` an advance,
anything else a message. When the worker ends, its stderr goes to `logs/<step>.log`; a non-zero
exit is an error quoting the last 12 lines. Otherwise the worker's measure file and the VRAM peaks
become the step's `StepMeasure`, with the device's free memory before the step and its growth
during it as notes.

## Boundaries

- Depends on: `child_process` (`Run`, `Running`), `job_model` (`StepMeasure`, `WorkerMeasure`),
  `crate::graph`, `crate::measure::gpu_monitor`, `crate::progress` and `crate::work_dir`.
- Used by: `crate::runner` for every step placed in a worker;
  `apps/tbd_subtitles/src/cli/process_command.rs` for `Binaries`.
- Rules:
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

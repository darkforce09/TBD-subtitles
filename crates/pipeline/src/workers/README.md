# Worker processes

Starting a step as a [worker process](/documentation/glossary.md#worker-process) of one of the
app's three binaries, sending its inputs, reading its frames, forwarding its progress, keeping its
outputs uncommitted and its stderr, and turning what it reports into the step's measure.

## Contents

```text
crates/pipeline/src/workers/
├── channel/       a step's inputs down the worker's stdin, its outputs into the job database
├── frames.rs      `read_frames`: a worker's stdout frames as progress events, outputs and its `WorkerReport`
├── gpu_lock.rs    the machine-wide GPU lock: one GPU worker at a time, audio steps first, naming this process's holder
├── lazy_gpu.rs    the lock taken in the worker when its model loads: `LOCK_VARIABLE`, `open_held`, `HeldModel`
├── mod.rs         `Binaries`, `WorkerData`, `WorkerRun` and `run_worker`
├── tests/         unit tests for the worker frames, the verdict, a missing binary, the GPU lock, the lazy lock and the memory wait
└── vram_guard.rs  the wait for the step's GPU memory, with its 10-minute deadline
```

## How it works

`Binaries::beside_current_exe` finds `tbd-subtitles`, `tbd-subtitles-ggml` and
`tbd-subtitles-llm` in the running binary's folder. `run_worker` takes a `WorkerData`: the job's
`JobStore`, whose work directory the worker runs in, and the addresses of the stored values the
step reads. It starts `<binary> worker <step> <job dir>` through `child_process::Run::spawn` with
the step's timeout from `graph`, the environment the runner passes (the CUDA runtime for GPU
steps) and the job's cancel token, which the child's watchdog watches: a cancelled worker is killed with its process
group and the step fails as cancelled. A GPU step first takes `gpu_lock`, an exclusive `flock` on
`gpu.lock` in the app data folder, on a descriptor of its own, waiting (and saying so once) while
another holder has it: another process of the app, or another GPU step of this one (the visual
lane's and the main walk's). A process-wide table, changed under the same mutex as the `flock`
calls, names the step and job holding each lock this process holds, so the waiting line
(`gpu_lock::waiting_message`) says "waiting for the GPU: text_detect of this job is using it",
"… of another job …", or "another run of the app is using it" when no step of this process holds
it; the kernel drops the lock when its holder dies. Each acquirer has the priority
`graph::gpu_priority` gives its step: an `Audio` acquirer (the main walk) that has to wait holds a
shared `flock` on `gpu.lock.audio` until its wait ends, and a `Visual` acquirer (the visual lane)
that cannot take that file exclusively keeps waiting, so a main-walk step waiting in this process
or another goes before every visual-lane step, even one that waited longer.

With the lock held, `vram_guard::wait_for_memory` reads the device's free memory through NVML
every second until it reaches the step's need (`graph::vram_need_mib`: its measured peak plus
256 MiB). While it waits the step shows "waiting for GPU memory: N MiB free, M needed", once and
again whenever the free memory moves by 128 MiB or more; cancelling the job ends the wait, and after
10 minutes the step fails naming the free memory and the need, with "close other GPU programs and
retry". Without NVML there is no check. The source of free memory is the `FreeMemory` trait, so
the tests use a fake.

A step where `graph::locks_gpu_lazily` (the translation, whose local model loads only for what
Claude leaves) takes neither up front: `run_worker` names the lock file to its worker in
`TBD_SUBTITLES_GPU_LOCK` (`lazy_gpu::LOCK_VARIABLE`, from `lazy_gpu::worker_variable`), and the
worker's `lazy_gpu::open_held` takes the lock with the step's priority and the memory wait just
before it opens the model, sending its waiting lines as `Message` frames. The model comes back as
a `HeldModel`, which implements `LanguageModel` and owns the lock: the model drops first, then the
lock. The worker's wait has no cancel token of its own; the runner stops it by killing the worker,
which frees the `flock`. A worker without the variable takes no lock.

For every GPU step `run_worker` reads the device's memory first and starts a
`measure::gpu_monitor::Monitor` on the worker's pid. A step with inputs gets a piped
stdin, which `channel::inputs::send_inputs` fills from one read snapshot on a thread of its own;
without inputs the worker's stdin is `/dev/null`. The runner passes every value `graph::reads`
gives the step (less the optional ones the job does not have), so every worker has at least the
job record.

The worker's stdout carries only frames of the worker channel (`crates/worker_channel/`), and
`frames::read_frames` reads them as bytes, never as lines:

| Frame | What the runner does |
|---|---|
| `Progress` | `Progress::StepAdvanced` with the step's done and total units |
| `ModelCall` | `Progress::ModelCall` from the call's JSON; JSON that does not parse is a short message naming its size |
| `Message` | `Progress::StepMessage` with its text, bad UTF-8 replaced: the worker's waiting lines |
| `Measure` | kept as the worker's `WorkerMeasure`, read from its rkyv archive with a check |
| `Failed` | kept as the worker's own reason, bad UTF-8 replaced |
| `Done` | the worker's end; any frame after it breaks the protocol |
| `Output` | its address read from the frame, then its archive read straight into the step's `channel::StepWrite`, which checks it; without a `StepWrite` it is a protocol error |
| `Input` | a protocol error: only the runner sends inputs |

An `Output` payload is never buffered: `read_frames` reads the address from a reader bounded by
the frame's length and hands the rest to the `StepWrite`, which must take all of it. A protocol
error (also an output the `StepWrite` refuses, a frame shorter than its address, a measure that
does not check, a stream cut inside a frame or an unknown tag) drops the step's outputs and kills
the worker at once (`Running::kill_and_wait`), writes its stderr to `logs/<step>.log`
and fails the step with the error and the log's last 12 lines. Its stderr lines are logged as they
arrive (`child_process`), and when the worker ends its stderr goes to `logs/<step>.log` too. A
non-zero exit is an error quoting the last 12 lines, preceded by the worker's `Failed` message
when it sent one; an exit 0 without `Measure` or without `Done` is an error naming what is
missing (`WorkerReport::verdict`, which decides from the report and the exit code); an input that
could not be sent fails a step that otherwise finished. Each of these drops the step's outputs
uncommitted. Otherwise the worker's measure and the VRAM peaks become the step's `StepMeasure`,
with the device's free memory before the step and its growth during it as notes, and `run_worker`
returns a `WorkerRun`: the measure, and the `StepWrite` of the outputs when the worker sent any,
which the caller commits with the step's record. The step's GPU lock, the free VRAM it starts
with and the path of its log file are debug `tracing` events. `channel/README.md` describes the
inputs and outputs.

## Boundaries

- Depends on: `child_process` (`Run`, `Running`), `worker_channel` (the frame codec, the address
  and `worker::message`), `inference::llm` (`LanguageModel`), `rkyv`, `serde_json`, `libc`
  (`flock`), `tracing`, `crate::cancel`, `job_model`
  (`StepMeasure`, `StepRecord`, `WorkerMeasure`, `ModelExchange`), `crate::graph`,
  `crate::measure::gpu_monitor`, `crate::progress` and `crate::work_dir` (`JobStore`, its rows and
  record kinds).
- Used by: `crate::runner` for every step placed in a worker, which commits the `StepWrite` a
  worker leaves; `tools/visual_validation/src/pilot.rs`; the translation task in
  `crate::tasks::onscreen` (`lazy_gpu::open_held`);
  the app's `process` and `fix` subcommands (`apps/tbd_subtitles/src/cli/`) and its window
  (`apps/tbd_subtitles/src/application/actions/runner.rs`) for `Binaries`.
- Rules:
  - one GPU worker runs at a time on the machine, two steps of one process included, the waiter
    learns which step of this process holds the lock, and a cancelled wait never takes the lock
    (`a_held_lock_waits_until_released_and_names_its_holder`, `a_released_lock_names_no_holder`,
    `a_cancelled_wait_gives_up` in `tests/gpu_lock.rs`);
  - the waiting line names a holder of this process, of this job or another, and otherwise
    another run of the app (`the_waiting_line_names_a_holder_of_this_process`);
  - a main-walk step waiting for the lock goes before a visual-lane step that waited longer, a
    visual-lane step takes a free lock once no main-walk step waits, and a cancelled main-walk
    waiter leaves no mark (`an_audio_waiter_goes_before_a_visual_one_that_waited_longer`,
    `a_visual_waiter_takes_the_lock_once_no_audio_waiter_is_left`, `a_cancelled_wait_gives_up`);
  - a step starts at once when its memory is free, waits saying so once and on each change of
    128 MiB, fails after its deadline naming the memory and the remedy, gives up when cancelled,
    and is not checked without NVML
    (`enough_free_memory_starts_at_once_and_says_nothing`,
    `a_short_card_waits_saying_so_once_and_on_each_meaningful_change`,
    `a_wait_past_the_deadline_fails_naming_the_memory_and_the_remedy`, `a_cancelled_wait_gives_up`,
    `without_nvml_there_is_no_check`, `the_deadline_is_ten_minutes_said_in_words` in
    `tests/vram_guard.rs`);
  - only a lazily locking step's worker is told the lock file; its model opens under the lock and
    after its memory wait, drops before the lock is released, and releases it when it fails to
    open (`only_a_lazily_locking_step_gets_the_lock_variable`,
    `the_model_opens_under_the_lock_and_drops_before_it_is_released`,
    `a_model_that_fails_to_open_releases_the_lock`, `the_model_waits_for_its_memory_saying_so`,
    `a_held_model_is_what_the_translation_opener_hands_back` in `tests/lazy_gpu.rs`);
  - a missing binary fails with where to look (`a_missing_binary_fails_naming_it` in
    `tests/workers.rs`);
  - a progress frame becomes an advance and a model call frame a model call, and a model call
    that does not parse a short message, never its bytes (`a_progress_frame_becomes_an_advance`,
    `a_model_call_frame_becomes_a_model_call`, `an_unreadable_model_call_is_a_short_message` in
    `tests/frames.rs`);
  - a message frame becomes a step message, its bad UTF-8 replaced
    (`a_message_frame_becomes_a_step_message`), and a failure message that is not UTF-8 is kept
    (`a_failure_that_is_not_text_is_kept`);
  - a frame after `Done`, a stream cut inside a frame, an unknown tag, an output with no step
    write to take it, an input or a measure that does not check breaks the protocol
    (`a_frame_after_the_end_breaks_the_protocol`, `a_stream_cut_inside_a_frame_breaks_the_protocol`,
    `an_unknown_tag_breaks_the_protocol`, `an_output_or_a_bad_measure_breaks_the_protocol`);
  - a worker finishes only with exit 0, its `Measure` and its `Done`, and otherwise fails naming
    what is missing after its own `Failed` message
    (`a_worker_finishes_only_with_exit_zero_its_measure_and_its_end` in `tests/frames.rs`);
  - a worker that exits non-zero, breaks the protocol, or exits 0 without `Measure` and `Done` is
    a failed step with the end of its stderr in the error, and its outputs are dropped (the module
    header of `mod.rs`; the channel's rules in `channel/README.md`);
  - the worker runs to its end on the calling thread, which the child dies with
    (`crates/child_process/src/lib.rs`).

## Related documentation

- [Each GPU stage runs in its own worker process](/documentation/decisions/foundations.md#2026-09-25--each-gpu-stage-runs-in-its-own-worker-process)
  — why models load in workers.
- [Each native GPU runtime lives in a worker binary of its own](/documentation/decisions/stack_and_pipeline.md#2026-09-26--each-native-gpu-runtime-lives-in-a-worker-binary-of-its-own)
  — why there are worker binaries.
- [Each GPU worker stays within 6.5 GB of VRAM](/documentation/decisions/foundations.md#2026-10-01--each-gpu-worker-stays-within-65-gb-of-vram-and-a-step-waits-up-to-a-deadline-for-the-memory-it-measured)
  — the memory wait, its deadline, audio first and the lazy lock.
- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#worker-channel) — the
  worker channel's frames and where they lead.

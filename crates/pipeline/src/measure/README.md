# Measurements

What each step costs: the VRAM a worker process holds, sampled through NVML while it runs; the peak
resident memory of a process and of the children it waited for; and what the job's whole process
tree and the GPU used while each step ran (CPU cores, the busiest thread, SM, NVENC and NVDEC use,
and the memory of every process of the job at once).

## Contents

```text
crates/pipeline/src/measure/
├── gpu_monitor.rs   the device and its memory now, and a thread sampling one pid's VRAM every 100 ms
├── job_sampler.rs   `JobSampler`: one thread sampling the job's process tree and the GPU every 250 ms
├── memory.rs        `VmHWM` of this process, `ru_maxrss` of its children, and the peak reset
├── mod.rs           the module tree
├── process_tree.rs  `/proc` parsers and readers: descendants, summed PSS, process and thread ticks
├── step_use.rs      `Sample`, CPU use between two readings, and the per-step windows (`StepUse`)
└── tests/           unit tests for the `/proc` parsers, the CPU maths, the windows and the sampler
```

## How it works

The runner starts one `JobSampler` with its own pid before it walks the steps. Every 250 ms the
sampler's thread finds every pid that descends from the app by walking each `/proc/<pid>/stat`'s
parent, sums their `Pss:` from `smaps_rollup` (proportional, so memory the app and its workers
share counts once), reads each process's and each thread's user and kernel ticks, and asks NVML for
the device's SM, encoder and decoder use. The CPU use of a sample is the ticks since the previous
sample over the time between them: a pid or thread new since then counts its ticks in full (it
started within the interval), and one gone since then loses the ticks of its last interval, at
most 250 ms of its life. The busiest thread's share is the largest per-thread delta.

The runner opens a step's window just before the step runs and closes it right after; each sample
is folded into every window open at the time (the shot scan's overlaps the steps beside it) and
into the job's own, keeping sums, counts and peaks, never the samples. Closing a window gives the
step's `StepUse`: mean GPU, NVENC and NVDEC use, the peak whole-job PSS, the mean and peak CPU cores,
and the busiest thread's mean share, which the runner puts into the step's `StepMeasure`. Stopping
the sampler gives the run's peak PSS for `meta/last_run`.

## Boundaries

- Depends on: `nvml-wrapper`, which loads the driver's `libnvidia-ml.so` at run time; `libc` for
  `getrusage(RUSAGE_CHILDREN)` and `sysconf(_SC_CLK_TCK)`; `/proc/self/status`,
  `/proc/self/clear_refs`, `/proc/<pid>/stat`, `/proc/<pid>/smaps_rollup` and
  `/proc/<pid>/task/<tid>/stat`; `serde` for `VramPeaks`; `job_model::job::StepMeasure`.
- Used by: `crate::workers` (the VRAM monitor around each GPU worker), `crate::tasks` (peak RAM in
  process and in a worker) and `crate::runner` (the job sampler around every step);
  `tools/stack_spike/src/measure/`; the app's machine check (`device_info`, in
  `apps/tbd_subtitles/src/settings/services/system_check.rs`).
- Rules:
  - without NVML, as in the development container, VRAM and GPU use are reported as not measured,
    never as zero (the headers of `gpu_monitor.rs` and `job_sampler.rs`);
  - `reset_peak_ram` runs before an in-process step, so its peak is that step's alone, and a
    refused reset records no peak (`crates/pipeline/src/tasks/mod.rs`);
  - GPU use is the whole device's, the desktop's share included, and a step's CPU and memory are
    the whole job's while it ran, the shot scan's alongside included;
  - a name with spaces or parentheses parses
    (`a_name_with_spaces_and_parentheses_parses_from_its_last_parenthesis` in
    `tests/process_tree.rs`), a new pid counts in full and a gone one not at all
    (`a_new_pid_counts_in_full_and_a_gone_one_counts_nothing` in `tests/step_use.rs`), and
    overlapping windows each take the samples while open
    (`overlapping_windows_each_take_the_samples_while_open`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#hardware-and-host-rules) — the
  GPU and memory budget the measures are checked against.
- [Pipeline](/documentation/architecture/pipeline.md) — the step records and the report the
  measures go into.

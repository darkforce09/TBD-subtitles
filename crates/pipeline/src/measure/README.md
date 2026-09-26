# Measurements

The peak memory of each step: the VRAM a worker process holds, sampled through NVML while it runs,
and the peak resident memory of a process and of the children it waited for.

## Contents

```text
crates/pipeline/src/measure/
├── gpu_monitor.rs  device memory now, and a thread sampling one pid's VRAM every 100 ms
├── memory.rs       `VmHWM` of this process, `ru_maxrss` of its children, and the peak reset
└── mod.rs          the module tree
```

## Boundaries

- Depends on: `nvml-wrapper`, which loads the driver's `libnvidia-ml.so` at run time; `libc` for
  `getrusage(RUSAGE_CHILDREN)`; `/proc/self/status` and `/proc/self/clear_refs`; `serde` for
  `VramPeaks`.
- Used by: `crate::workers` (the VRAM monitor around each GPU worker) and `crate::tasks` (peak RAM
  in process and in a worker); `tools/stack_spike/src/measure/`.
- Rules:
  - without NVML, as in the development container, VRAM is reported as not measured, never as
    zero (the header of `gpu_monitor.rs`);
  - `reset_peak_ram` runs before an in-process step, so its peak is that step's alone, and a
    refused reset records no peak (`crates/pipeline/src/tasks/mod.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#hardware-and-host-rules) — the
  GPU and memory budget the measures are checked against.

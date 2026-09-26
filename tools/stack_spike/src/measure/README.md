# Measurement

Measuring one stack item: the parent checks the VRAM budget, starts the item in a worker process
with the CUDA runtime on its library path, samples its VRAM through the job runner's NVML
monitor and records the result; the worker runs the item and reports its own times and memory
peaks.

## Contents

```text
tools/stack_spike/src/measure/
├── mod.rs     the parent side: budget check, spawn, VRAM sampling, stdout forwarding, `ItemResult`
└── worker.rs  the child side: run the item, then write times, `VmHWM` and the children's peak
```

## Boundaries

- Depends on: `child_process::Run::spawn`, `inference::cuda_runtime`,
  `pipeline::measure::gpu_monitor` (the device's free memory, `Monitor` and `VramPeaks`) and
  `pipeline::measure::memory` (`VmHWM` and the children's `ru_maxrss`).
- Used by: `tools/stack_spike/src/main.rs` (`run`, `worker`) and `report.rs` (`read_results`).
- Rules: a GPU item runs only with `VRAM_BUDGET_MIB` free and NVML present, otherwise it is
  recorded as not run; a worker that exits non-zero or writes no report is recorded as failed
  with its last stderr lines (the header in `mod.rs`).

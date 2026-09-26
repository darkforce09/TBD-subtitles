# Progress events

What a running job reports: the job's start, and each step skipped, started, advanced, printing a
line or finished with its measure. The command line prints these events.

## Contents

```text
crates/pipeline/src/progress/
└── mod.rs  the `Progress` event and the `ProgressSink` it is sent to
```

## Boundaries

- Depends on: `job_model` (`StepName`, `StepMeasure`).
- Used by: `crate::runner` and `crate::workers`, which send the events;
  `apps/tbd_subtitles/src/cli/process_command.rs`, which prints them.
- Rules: a sink is `Sync`, since the runner's thread and the shot-scan thread both call it.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — where progress is shown.
- [Automation](/documentation/features/automation.md) — the headless runs that report it.

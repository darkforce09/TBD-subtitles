# Progress events

Progress events from a running job: the current stage, its progress, and the elapsed and remaining
time. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/pipeline/src/progress/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/pipeline/src/lib.rs` declares it as a public module.
- Rules: none of its own beyond the crate's; see the
  [crate README](/crates/pipeline/README.md#boundaries).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — where progress is shown.
- [Automation](/documentation/features/automation.md) — the headless runs that report it.

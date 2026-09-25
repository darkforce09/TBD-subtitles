# Resume

Skipping a [stage](/documentation/glossary.md#stage) whose output exists and whose recorded inputs
and settings are unchanged. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/pipeline/src/resume/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/pipeline/src/lib.rs` declares it as a public module.
- Rules: a killed job leaves every finished stage valid for resume (the crate header in
  `crates/pipeline/src/lib.rs`), and a stage's output is complete or absent, never partial (the
  crate header in `crates/stages/src/lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — when a
  stage is skipped and how deleting an output reruns it.

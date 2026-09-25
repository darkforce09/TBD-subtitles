# Work directory

The job's work directory: its layout, the path of each stage's output, and `job.json`. The module's
code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/pipeline/src/work_dir/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/pipeline/src/lib.rs` declares it as a public module.
- Rules: none of its own beyond the crate's; see the
  [crate README](/crates/pipeline/README.md#boundaries).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the
  layout of a job's work directory.

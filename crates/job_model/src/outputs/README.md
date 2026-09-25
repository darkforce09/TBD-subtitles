# Stage outputs

The typed output of each [stage](/documentation/glossary.md#stage), one JSON file per stage in the
job's work directory. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/job_model/src/outputs/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/job_model/src/lib.rs` declares it as a public module.
- Rules: an output type changes only together with every stage that reads or writes it, and its
  JSON names stay stable so a resumed job reads what an earlier run wrote (the crate header in
  `crates/job_model/src/lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — the file
  each stage writes.
- [Pipeline](/documentation/architecture/pipeline.md) — what each stage produces.

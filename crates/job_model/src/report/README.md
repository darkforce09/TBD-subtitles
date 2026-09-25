# Job report

The job report: the quality-check results, the flagged lines with their timestamps, and each
stage's time and peak memory. The module's code is not written yet; `mod.rs` holds only its header.

## Contents

```text
crates/job_model/src/report/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/job_model/src/lib.rs` declares it as a public module.
- Rules: once written, the report's JSON names stay stable so a resumed job reads what an earlier
  run wrote (the crate header in `crates/job_model/src/lib.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — the checks the report
  records.
- [Desktop GUI](/documentation/features/gui.md) — where the report is shown.

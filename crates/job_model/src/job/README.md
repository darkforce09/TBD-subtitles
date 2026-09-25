# Job record

The job record: one video, its probe result, its settings, and the status and timing of every
stage, as kept in the job's `job.json`. The module's code is not written yet; `mod.rs` holds only
its header.

## Contents

```text
crates/job_model/src/job/
└── mod.rs  the module header; no items yet
```

## Boundaries

- Depends on: nothing; the module holds no code.
- Used by: nothing; `crates/job_model/src/lib.rs` declares it as a public module.
- Rules: once written, the record's JSON names stay stable so a resumed job reads what an earlier
  run wrote (the crate header in `crates/job_model/src/lib.rs`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md#job-work-directory) — `job.json`
  among the files of a job's work directory.

# Job report

The job's `report.md`: the quality check and the job record rendered as Markdown, written after
every run so its step table shows the latest times.

## Contents

```text
crates/pipeline/src/report/
└── mod.rs  `write`: read `qc.json` and the dropped sounds, render, write `report.md`
```

## Boundaries

- Depends on: `stages::qc::markdown::render` and `stages::output::subtitle_path`; `job_model`
  (`JobRecord`, `QcReport`); `crate::work_dir`.
- Used by: `crate::runner`, at the end of every job, skipped steps included.
- Rules: the report is written through a part file like every job file, and a missing dropped
  sounds file counts as none.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — what the quality check
  holds.

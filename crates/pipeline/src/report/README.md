# Job report

The job's `report.md`: the quality check and the job record rendered as Markdown, written after
every run so its step table shows the latest times.

## Contents

```text
crates/pipeline/src/report/
├── mod.rs  `write`: the stored check and dropped sounds rendered, the on-screen text section added
└── tests/  the stored documents it reads, the localized-video lines and when they appear
```

## How it works

`write` reads one snapshot of the job's store through the process's handle of it
(`JobStore::open`, shared with the runner) and renders the quality check (`outputs/qc`) and the
dropped sounds (`outputs/cues/dropped_sounds`) with `stages::qc::markdown::render`. With on-screen text on, it
adds the on-screen text section from `outputs/text_typeset`: the counts, the visual processing
time, the review warnings and every occurrence with a warning or no English. When the
localized-video step ran without recording itself disabled, a `Localized video` subsection
follows: the occurrences replaced in the video (from `outputs/localized_video`), the
occurrences left in Japanese (from `outputs/text_verify`, after the read-back check), the
localized video's path and the encoder that wrote it.

## Boundaries

- Depends on: `stages::qc::markdown::render` and `stages::output::subtitle_path`; `job_model`
  (`JobRecord`, `QcReport`, `TextDocument`, `VerifiedReplacements`, `LocalizedVideoRecord`);
  `crate::work_dir` (the store and its keys); `rkyv` (the checked reads).
- Used by: `crate::runner`, at the end of every job, skipped steps included.
- Rules: the report is written through a part file like every job file; a document it reads and
  does not find is an error naming its key
  (`the_report_renders_the_stored_check_and_dropped_sounds_and_names_a_missing_one` in
  `tests/report.rs`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — what the quality check
  holds.

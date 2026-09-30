# Job report

The job's `report.md`: the quality check and the job record rendered as Markdown, written after
every run so its step table shows the latest times.

## Contents

```text
crates/pipeline/src/report/
├── mod.rs  `write`: read `qc.json` and the dropped sounds, render, add the on-screen text section, write `report.md`
└── tests/  the localized-video lines and when they appear
```

## How it works

`write` renders the quality check with `stages::qc::markdown::render`. With on-screen text on, it
adds the on-screen text section from `visual/text_typeset.json`: the counts, the visual processing
time, the review warnings and every occurrence with a warning or no English. When the
localized-video step ran without recording itself disabled, a `Localized video` subsection
follows: the occurrences replaced in the video (from `visual/localized_video.json`), the
occurrences left in Japanese (from `visual/text_verify.json`, after the read-back check), the
localized video's path and the encoder that wrote it.

## Boundaries

- Depends on: `stages::qc::markdown::render` and `stages::output::subtitle_path`; `job_model`
  (`JobRecord`, `QcReport`, `TextDocument`, `ReplacementDocument`, `LocalizedVideoRecord`);
  `crate::work_dir`.
- Used by: `crate::runner`, at the end of every job, skipped steps included.
- Rules: the report is written through a part file like every job file, and a missing dropped
  sounds file counts as none.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#10-quality-check) — what the quality check
  holds.

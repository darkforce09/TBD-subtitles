**Status:** live

# Decisions

The log of choices that shape TBD-subtitles and would otherwise be argued again. Each entry says
what was decided, why, what follows, and which earlier entry it replaces. A changed decision gets a
new entry that names the old one under Supersedes. Open questions that are not yet decided live in
the [roadmap](/documentation/roadmap.md#open-questions).

## Contents

```text
documentation/decisions/
├── automation.md          watch folders, the one window, the Dolphin entry, notifications, the icon
├── batch.md               running many videos at once: Fix It on the whole batch under one cap
├── desktop_gui.md         the window, its queue, report, review and settings, and the batch
├── foundations.md         what is built, in what language, with which programs, where files go
├── onscreen_detection.md  how the detection step screens proxies: batch, arena, which detector
├── stack_and_pipeline.md  the runtimes, models and binaries, the pipeline's steps, the pilot
└── storage.md             the job database, archived values, the owning process, the worker channel, the sign library
```

## How it works

The log is one set of entries split over files by subject, each file within 500 lines. An entry is
a `###` heading, `<date> — <the decision>`, with Context, Decision, Consequences and Supersedes in
that order, written from the [decision entry template](/documentation/standards/templates/decisions_entry.md)
and added at the end of the file of its subject. A link to an entry names its file and heading
anchor. Entries keep their words once written: a decision that changes gets a new entry.

## Boundaries

- Depends on: the owner's rulings and the measurements in the
  [research](/documentation/research/) snapshots.
- Used by: CLAUDE.md, the roadmap, the architecture documents and the code READMEs, which cite
  entries by link.
- Rules: every file here is a live document with its status line (`cargo gates status-lines`),
  within 500 lines (`cargo gates markdown-placement`); only these files may use the history words
  (`cargo gates prose-rules`).

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the milestones and the open questions not yet decided.
- [Documentation standards](/documentation/standards/documentation_standards.md) — the document
  kinds and their lifecycle.

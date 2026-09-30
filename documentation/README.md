**Status:** live

# Documentation

Every document of TBD-subtitles: the goals, the decisions, the roadmap, the architecture, the
research behind it, the planned features, the runbooks and the standards that shape the code and
these pages. Start here to find the document on a subject and to learn which source wins when two
disagree.

## Contents

```text
documentation/
├── architecture/        how the app is built: system overview, pipeline, subtitle style rules
├── decisions/           dated decision log, one file per subject
├── features/            one document per user-facing feature: GUI, automation, Japanese text
├── glossary.md          the project's terms and abbreviations
├── optimizations/       dialogue accuracy, visual tracking, fast re-encoding, memory scaling
├── research/            dated research snapshots: speech recognition and the Rust ML stack
├── roadmap.md           milestones with checklists, acceptance tests and open questions
├── runbooks/            procedures: development environment, continuing in Claude Code
├── standards/           documentation, README, coding and commit rules, and the templates
└── vision_and_goals.md  main goal, goals, non-goals, success criteria, performance budget
```

## How it works

The project runs on a small version of the TBD-Reforger `documentation_v2` system. Documents live
only in this folder; each code folder carries its own README.md (what the folder holds, how it fits
together, where it stops), written to the [README standard](/documentation/standards/readme_standard.md).
A document about one part of the code sits at the code's path with `apps/`, `crates/` and `src/`
left out, so the app's feature folder `apps/tbd_subtitles/src/job_queue/` would be documented in
a folder named `tbd_subtitles/job_queue/` here; such folders are created when the first document
about that code is written.

Every document opens with a status line. A **live** document tracks the code and changes in the
same commit as the code it describes. A **frozen record** (a dated research snapshot) keeps its
words; when the facts move on, a new snapshot is written and the old one stays. The
[documentation standards](/documentation/standards/documentation_standards.md) set the rules, and
`cargo gates` checks the ones a program can check.

### Authority ladder

When two sources disagree, the higher one wins and the lower one is corrected:

1. The running code.
2. [CLAUDE.md](/CLAUDE.md): the project laws, the directory atlas and the environment rules.
3. [Decisions](/documentation/decisions/): the latest entry on a subject.
4. This README and the [standards](/documentation/standards/README.md).
5. Live documents: architecture, features, roadmap, runbooks.
6. Frozen research snapshots under `research/`.

### Where things live

| To find | Look in |
|---|---|
| what the app is for and how good and fast it must be | [vision_and_goals.md](/documentation/vision_and_goals.md) |
| what to work on next | [roadmap.md](/documentation/roadmap.md) |
| why something is the way it is | [decisions](/documentation/decisions/) |
| the processes, crates and data flow | [architecture/](/documentation/architecture/README.md) |
| each stage from video to subtitle file | [architecture/pipeline.md](/documentation/architecture/pipeline.md) |
| how subtitles must look and be timed | [architecture/subtitle_style_rules.md](/documentation/architecture/subtitle_style_rules.md) |
| optimization designs, audio accuracy and memory scaling | [optimizations/](/documentation/optimizations/README.md) |
| which Rust crates and model files to use | [research/rust_ml_stack.md](/documentation/research/rust_ml_stack.md) |
| a planned feature's behaviour | [features/](/documentation/features/README.md) |
| how to set up, build, run or hand over | [runbooks/](/documentation/runbooks/README.md) |
| the rules for code, READMEs, documents and commits | [standards/](/documentation/standards/README.md) |
| a skeleton to start a README or document from | [standards/templates/](/documentation/standards/templates/README.md) |
| a term or abbreviation | [glossary.md](/documentation/glossary.md) |

## Code

- [The app](/apps/README.md) — the `tbd-subtitles` binary.
- [Library crates](/crates/README.md) — the job model, child processes, media input, subtitle
  formats, inference, stages and the job runner.
- [Repository tools](/tools/README.md) — the `cargo gates` runner and its check library.

## Boundaries

- Depends on: the code it describes, and the laws in [CLAUDE.md](/CLAUDE.md).
- Used by: every AI session and person working on the project; CLAUDE.md and the root README
  link here first.
- Rules: every folder here has a README.md whose Contents matches it (`cargo gates
  readme-coverage`); every document opens with its status line (`cargo gates status-lines`);
  every link and backticked path resolves (`cargo gates link-check`); a live document stays within
  500 lines (`cargo gates markdown-placement`).

## Related documentation

- [CLAUDE.md](/CLAUDE.md) — project laws and environment rules.
- [Documentation standards](/documentation/standards/documentation_standards.md) — layout, names,
  status lines and lifecycle.

**Status:** live

# Documentation

Every document of TBD-subtitles: the goals, the decisions, the roadmap, the architecture, the
research behind it, the planned features, the runbooks and the standards that shape the code and
these pages. Start here to find the document on a subject and to learn which source wins when two
disagree.

## Contents

```text
documentation/
├── README.md              this map
├── glossary.md            the project's terms and abbreviations
├── vision_and_goals.md    main goal, goals, non-goals, success criteria, performance budget
├── decisions.md           dated decision log
├── roadmap.md             milestones M0–M4 with checklists and open questions
├── architecture/          how the app is built: system overview and the subtitle pipeline
├── research/              dated research snapshots and the subtitle style rules
├── features/              one document per user-facing feature: GUI, automation, Japanese text
├── runbooks/              procedures: development environment, continuing in Claude Code
└── standards/             documentation, coding and commit rules
```

## How it works

The project runs on a small version of the TBD-Reforger `documentation_v2` system. Documents live
only in this folder, never beside the code. Once code exists, each code folder carries its own
README.md (what the folder holds, how it fits together, where it stops) and this tree goes deeper;
a document about code sits at the code's path with `apps/` and `src/` left out, so
`apps/tbd_subtitles/src/job_queue/` is documented in `documentation/tbd_subtitles/job_queue/`.

Every document opens with a status line. A **live** document tracks the code and changes in the
same commit as the code it describes. A **frozen record** (a dated research snapshot) keeps its
words; when the facts move on, a new snapshot is written and the old one stays. The
[documentation standards](/documentation/standards/documentation_standards.md) set the rules.

### Authority ladder

When two sources disagree, the higher one wins and the lower one is corrected:

1. The running code.
2. [CLAUDE.md](/CLAUDE.md): the project laws, the directory atlas and the environment rules.
3. [Decisions](/documentation/decisions.md): the latest entry on a subject.
4. This README and the [standards](/documentation/standards/README.md).
5. Live documents: architecture, features, roadmap, runbooks.
6. Frozen research snapshots under `research/`.

### Where things live

| To find | Look in |
|---|---|
| what the app is for and how good and fast it must be | [vision_and_goals.md](/documentation/vision_and_goals.md) |
| what to work on next | [roadmap.md](/documentation/roadmap.md) |
| why something is the way it is | [decisions.md](/documentation/decisions.md) |
| the processes, crates and data flow | [architecture/](/documentation/architecture/README.md) |
| each stage from video to subtitle file | [architecture/pipeline.md](/documentation/architecture/pipeline.md) |
| which Rust crates and model files to use | [research/rust_ml_stack.md](/documentation/research/rust_ml_stack.md) |
| how subtitles must look and be timed | [research/subtitle_style_rules.md](/documentation/research/subtitle_style_rules.md) |
| a planned feature's behaviour | [features/](/documentation/features/README.md) |
| how to set up, build, run or hand over | [runbooks/](/documentation/runbooks/README.md) |
| the rules for code, documents and commits | [standards/](/documentation/standards/README.md) |
| a term or abbreviation | [glossary.md](/documentation/glossary.md) |

## Related documentation

- [CLAUDE.md](/CLAUDE.md) — project laws and environment rules.
- [Documentation standards](/documentation/standards/documentation_standards.md) — layout, names,
  status lines and lifecycle.

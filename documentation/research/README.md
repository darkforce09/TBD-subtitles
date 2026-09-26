**Status:** live

# Research

The research behind the design: dated snapshots of the speech-recognition field and of the Rust
crates and model files that can run each capability, and the measurements of that stack on a real
episode.

## Contents

```text
documentation/research/
├── long_video_120min.md             frozen record, 2026-09-26: a 128.9-minute video, time and memory
├── pilot_dressrosa_11.md            frozen record, 2026-09-26: the first end-to-end run, its fixes, resume
├── rust_ml_stack.md                 frozen record, 2026-09-25: Rust crates and model files per capability
├── speech_recognition_landscape.md  frozen record, 2026-09-25: benchmarks, prices, options, why local
└── stack_spike_dressrosa_11.md      frozen record, 2026-09-26: every stack piece measured on one episode
```

## How it works

A snapshot is a frozen record: it keeps its words, because it says what was true on its date.
When the facts move on (a new model, a new crate release, measurements on real audio), a new
snapshot is written from the [research snapshot template](/documentation/standards/templates/research_snapshot.md),
named with its subject and date, and listed here; decisions that change get a new entry in the
[decision log](/documentation/decisions/).

## Code

- [Inference backends](/crates/inference/) — the crate that runs the models these snapshots
  choose.
- [Pipeline stages](/crates/stages/) — the stages each capability serves.

## Boundaries

- Depends on: the public sources each snapshot cites, and the research snapshot template.
- Used by: the [pipeline](/documentation/architecture/pipeline.md), the
  [system overview](/documentation/architecture/system_overview.md), the decision log and the
  roadmap, which cite the findings.
- Rules: every document here other than this index is a frozen record (`cargo gates
  status-lines`); a frozen record changes only to fix a broken link (`cargo gates link-check`
  judges its links alone).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — where each finding is used.
- [Decisions](/documentation/decisions/) — what the research led to.

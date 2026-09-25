**Status:** live

# Research

The research behind the design: dated snapshots of the speech-recognition field and of the Rust
crates that can run it, and the subtitle style rules the output must follow.

## Contents

```text
research/
├── README.md                          this index
├── speech_recognition_landscape.md    frozen record, 2026-09-25: benchmarks, prices, options, why local
├── rust_ml_stack.md                   frozen record, 2026-09-25: Rust crates and model files per capability
└── subtitle_style_rules.md            live: Netflix English SDH layout and timing rules, as applied here
```

## How it works

A snapshot is a **frozen record**: it keeps its words, because it says what was true on its date.
When the facts move on (a new model, a new crate release, measurements from the M0.5 spike), write
a new snapshot named with its subject and date and link it here; decisions that change get a new
entry in [decisions.md](/documentation/decisions.md). The style rules are **live** and change with
the cue builder.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — where each finding is used.
- [Decisions](/documentation/decisions.md) — what the research led to.

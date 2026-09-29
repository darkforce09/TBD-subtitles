# Visual validation source

Commands and fail-closed evaluation for local visual pilots.

## Contents

```text
tools/visual_validation/src/
├── evaluate.rs  coverage, timing and geometry verdicts
├── main.rs      fetch, still recognition, clip and evaluation commands
├── pilot.rs     measured six-stage pilots using production workers and resume
├── scenarios.rs  synthetic Japanese credits, lyric, vertical and brief-text source fixtures
└── tests/       evaluator regression cases
```

## How it works

Commands use production backends and write explicit outputs. Evaluation matches independent annotations to distinct observed occurrences and checks timing and geometry at source resolution.

## Boundaries

- Depends on: shared Rust application crates.
- Used by: the visual validation command.
- Rules: source videos are read-only and annotation failures never become passing verdicts.

## Related documentation

- [Validation tool](/tools/visual_validation/) — commands and configuration.

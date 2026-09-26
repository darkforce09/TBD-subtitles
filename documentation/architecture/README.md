**Status:** live

# Architecture

How TBD-subtitles is built: the processes, crates and data flow, the pipeline that turns a video
into a subtitle file, and the layout rules every subtitle file follows.

## Contents

```text
documentation/architecture/
├── pipeline.md              every stage from video to subtitle file, with its rules and guards
├── subtitle_style_rules.md  Netflix English SDH layout and timing rules, as the cue builder applies them
└── system_overview.md       processes, crates and layers, job work directory, models, configuration
```

## How it works

The [system overview](/documentation/architecture/system_overview.md) is the map: one binary with
a GUI, a CLI and worker subcommands; a job runner that runs stages in order; FFmpeg for all media
input. The [pipeline](/documentation/architecture/pipeline.md) is the detail of each stage, and the
[subtitle style rules](/documentation/architecture/subtitle_style_rules.md) fix how the cue
builder lays text out. The crate and model choices behind them come from the
[Rust ML stack](/documentation/research/rust_ml_stack.md) research.

## Code

- [The app](/apps/tbd_subtitles/) — the binary and its subcommands.
- [Library crates](/crates/) — the job model, media input, stages, inference and subtitle formats.

## Boundaries

- Depends on: the research snapshots and the decision log for the choices it describes.
- Used by: CLAUDE.md, the roadmap, the feature documents and the READMEs of the crates.
- Rules: the crate layers described here are the ones `cargo gates crate-layering` holds; each
  document is live and changes in the same commit as the code it describes.

## Related documentation

- [Vision and goals](/documentation/vision_and_goals.md) — the quality and speed targets.
- [Decisions](/documentation/decisions/) — why the design is shaped this way.
- [Roadmap](/documentation/roadmap.md) — the order in which it gets built.

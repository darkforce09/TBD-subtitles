**Status:** live

# Architecture

How TBD-subtitles is built: the processes, crates and data flow, and the pipeline that turns a
video into a subtitle file. These documents describe the design ahead of the code; once code
exists they describe the code as it stands.

## Contents

```text
architecture/
├── README.md             this index
├── system_overview.md    processes, planned crates, job work directory, models, configuration
└── pipeline.md           every stage from video to subtitle file, with its rules and guards
```

## How it works

The [system overview](/documentation/architecture/system_overview.md) is the map: one binary with
a GUI, a CLI and worker subcommands; a job runner that runs stages in order; FFmpeg for all media
input. The [pipeline](/documentation/architecture/pipeline.md) is the detail of each stage. The
crate and model choices behind both come from the
[Rust ML stack](/documentation/research/rust_ml_stack.md) research.

## Related documentation

- [Vision and goals](/documentation/vision_and_goals.md) — the quality and speed targets.
- [Decisions](/documentation/decisions.md) — why the design is shaped this way.
- [Roadmap](/documentation/roadmap.md) — the order in which it gets built.

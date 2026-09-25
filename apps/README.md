# Applications

The executables people run. The one application is the TBD-subtitles binary: the desktop window,
the headless command line and the [worker processes](/documentation/glossary.md#worker-process)
that run GPU stages, all in one program.

## Contents

```text
apps/
└── tbd_subtitles/  the `tbd-subtitles` binary: the window, the headless commands, the GPU workers
```

## How it works

An application composes the library crates under `crates/` and adds only what a person or a
process launcher touches: the command line, the window and the error report at the top. The
pipeline logic, the media handling and the subtitle formats live in those crates, never here.
`tbd_subtitles/README.md` describes the binary, and its `src/README.md` maps the modules.

```text
apps/tbd_subtitles ──▶ crates/pipeline ──▶ crates/stages ──▶ inference, media_io, subtitle_formats
       │                                                                │
       └────────────────────────▶ crates/job_model ◀─────────────────────┘
```

## Getting started

Run these from the repository root; the window needs a desktop session.

```bash
cargo run -p tbd_subtitles             # opens the window, empty queue; stays in the foreground
cargo run -p tbd_subtitles -- --help   # the subcommands: gui, process, worker
cargo test -p tbd_subtitles            # CLI, queue, rendering and architecture tests; headless
```

## Boundaries

- Depends on: `crates/pipeline/` and `crates/job_model/`, the library crates the binary declares.
- Used by: people at a desktop or a terminal; no crate links or starts an application.
- Rules: an application sits on the top layer, and no crate under `crates/` depends on one
  (`cargo gates crate-layering`); every folder here carries a README whose Contents block matches
  it (`cargo gates readme-coverage`).

## Related documentation

- [System overview](/documentation/architecture/system_overview.md) — the processes, crates and
  work directory the binary ties together.
- [Coding standards](/documentation/standards/coding_standards.md) — the layering and the feature
  folder layout the app follows.

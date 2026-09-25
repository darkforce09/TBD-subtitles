# TBD Subtitles

The `tbd_subtitles` crate, which builds the `tbd-subtitles` binary: the eframe desktop window that
queues videos, the headless `process` command, and the `worker` subcommand that runs one GPU
[stage](/documentation/glossary.md#stage) of a job in its own process. The owner runs it on the
host PC.

## Contents

```text
apps/tbd_subtitles/
├── Cargo.toml  the package: the `tbd-subtitles` binary and its commented dependencies
└── src/        the entry point, command line, window, shared core and four feature modules
```

## How it works

`src/main.rs` installs logging, parses the command line and runs the chosen subcommand. With no
subcommand, or with `gui`, it opens a 1100 by 700 window (640 by 400 at least) titled "TBD
Subtitles", drawn with eframe's glow renderer; videos named on the command line or dropped onto
the window join the queue on the left, skipping any already queued. `process` checks that each
named video is a readable file, and `worker` accepts only the stages that run in a
[worker process](/documentation/glossary.md#worker-process); no pipeline stage is built, so both
end with an error that says so rather than report success.

The source tree splits into composition (`cli`, `application`), shared foundations (`core`) and
feature folders (`job_queue`, `job_report`, `line_review`, `settings`), each feature with
`models/`, `services/` and `ui/`. The window lends each feature a borrowed view every frame and
applies the events it returns after the frame. `src/README.md` maps the modules.

## Getting started

Run these from the repository root; the window needs a desktop session.

```bash
cargo run -p tbd_subtitles                     # the window, empty queue; stays in the foreground
cargo run -p tbd_subtitles -- gui a.mkv b.mkv  # opens the window with these videos queued
cargo run -p tbd_subtitles -- process a.mkv    # checks the file; exits 1, as no stage is built
cargo run -p tbd_subtitles -- --help           # the usage and the three subcommands
```

Check the crate with:

```bash
cargo fmt -p tbd_subtitles --check
cargo clippy -p tbd_subtitles --all-targets -- -D warnings
cargo test -p tbd_subtitles                    # headless: no window opens
cargo gates file-length
```

## Configuration

- `RUST_LOG`: the log filter, read by `src/core/logging.rs`; `info` when unset or invalid. Log
  lines go to stderr, coloured only when stderr is a terminal.
- Build features: none of its own. The `eframe` dependency is built with `glow`, `wayland`, `x11`
  and `default_fonts` only, because wgpu fails to create a surface on the owner's Wayland desktop.
- No settings file is read: the `src/settings/` feature holds no code yet, and the window keeps
  no state between runs.

## Public surface

- The `tbd-subtitles` binary: `tbd-subtitles [COMMAND]`, with the subcommands `gui [VIDEOS]...`,
  `process <VIDEOS>...` and `worker <STAGE> <JOB_DIR>`, and `--help` and `--version`. It exits 0
  on success, 1 with the error chain on stderr, and 2 on a usage error. `src/cli/README.md`
  describes each subcommand. There is no library target.

## Boundaries

- Depends on: `crates/job_model/` for `StageName`; `crates/pipeline/`, declared in `Cargo.toml`
  and not yet called; the `anyhow`, `clap`, `eframe`, `tracing` and `tracing-subscriber` crates.
- Used by: people at a desktop or a terminal; no crate links it and nothing in the repository
  starts it.
- Rules:
  - the crate is the top product layer, and no crate depends on it (`cargo gates crate-layering`);
  - the source layout, the dependency directions and the file-size limits hold under
    `src/tests/architecture_rules.rs`, whose `source_inspection.rs` reads grouped imports and
    aliases and ignores comments and string literals; `cargo gates file-length` covers `src/` too;
  - a command that cannot do its work exits non-zero and never reports success
    (`process_and_worker_never_report_success_before_the_stages_exist` in
    `src/cli/tests/cli.rs`).

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue, progress, report, review and settings
  the window is built to show.
- [System overview](/documentation/architecture/system_overview.md) — the `gui`, `process` and
  `worker` processes and the crates behind them.
- [Development environment](/documentation/runbooks/development_environment.md) — the host, the
  container and where to run GPU work.

# TBD Subtitles

The `tbd_subtitles` crate, which builds the `tbd-subtitles` binary: the eframe desktop window that
queues videos, the headless `process` command that runs a job from video to subtitle file, and
the `worker` subcommand that runs one step of a job in its own
[worker process](/documentation/glossary.md#worker-process). The owner runs it on the host PC.

## Contents

```text
apps/tbd_subtitles/
├── Cargo.toml  the package: the `tbd-subtitles` binary and its commented dependencies
└── src/        the entry point, command line, window, shared core and four feature modules
```

## How it works

`src/main.rs` installs logging, parses the command line and runs the chosen subcommand. With no
subcommand, or with `gui`, it opens a 1100 by 700 window (640 by 400 at least) titled "TBD
Subtitles", drawn with eframe's glow renderer under X11 (XWayland on a Wayland desktop), in
Adwaita Sans and the desktop's light or dark colour scheme; videos named on the command line or dropped onto
the window join the queue on the left, skipping any already queued. The window runs no job.

`process` turns its options into the job settings, checks every video is a readable file, and
runs one job per video through `pipeline::run_job`, printing each
[step](/documentation/architecture/pipeline.md) as it starts, is skipped as still valid, advances
and finishes, then the subtitle file beside the video, the job's report and the quality summary.
The job runner starts each worker step as `tbd-subtitles worker <step> <job dir>`, or, for the
Whisper steps, as `tbd-subtitles-ggml worker <step> <job dir>` from the ggml worker in
`apps/tbd_subtitles_ggml/`, which must be built into the same folder as this binary.

The source tree splits into composition (`cli`, `application`), shared foundations (`core`) and
feature folders (`job_queue`, `job_report`, `line_review`, `settings`), each feature with
`models/`, `services/` and `ui/`. The window lends each feature a borrowed view every frame and
applies the events it returns after the frame. `src/README.md` maps the modules.

## Getting started

Run these from the repository root; the window needs a desktop session. A full `process` run
needs the host (FFmpeg, the GPU), the models and CUDA runtime in `~/.local/share/tbd-subtitles/`,
and `tbd-subtitles-ggml` built beside this binary (see `apps/tbd_subtitles_ggml/README.md`).

```bash
cargo run -p tbd_subtitles                     # the window, empty queue; stays in the foreground
cargo run -p tbd_subtitles -- gui a.mkv b.mkv  # opens the window with these videos queued
cargo run -p tbd_subtitles -- --help           # the usage and the three subcommands
cargo build --release -p tbd_subtitles
distrobox-host-exec target/release/tbd-subtitles process "<video>"   # subtitles beside the video
```

Check the crate with:

```bash
cargo fmt -p tbd_subtitles --check
cargo clippy -p tbd_subtitles --all-targets -- -D warnings
cargo test -p tbd_subtitles                    # headless: no window opens, no job runs
cargo gates file-length
```

## Configuration

- `RUST_LOG`: the log filter, read by `src/core/logging.rs`; `info` when unset or invalid. Log
  lines go to stderr, coloured only when stderr is a terminal.
- `XDG_DATA_HOME`, else `HOME`: the data folder `tbd-subtitles/` that holds the models, the CUDA
  runtime and, unless `--work-root` names another, the jobs' work directories under `work/`
  (read by `crates/inference/src/model_store/mod.rs` and `crates/pipeline/src/work_dir/mod.rs`).
- `XDG_CONFIG_HOME`, else `HOME`: the settings file `tbd-subtitles/settings.toml` under the
  config folder (`~/.config`), read by `src/settings/services/settings_file.rs` for
  the `process` subcommand. A missing file means the defaults; an unknown key or a bad value is an
  error naming it. Its keys: `models_dir`, `work_root`, `glossary`, `cut_score`, `output_format`,
  `[engines]` `separator` and `whisper`, `[language_model]` `backend`, `model` and `processes`.
- The `process` options (`--settings`, `--models-dir`, `--glossary`, `--audio-track`,
  `--separator`, `--whisper`, `--cut-score`, `--llm-model`, `--format`, `--rerun`), which win over
  the settings file: `src/cli/README.md`.
- Build features: none of its own. The `eframe` dependency is built with `glow`, `wayland`, `x11`
  and `default_fonts` only, because wgpu fails to create a surface on the owner's Wayland desktop.
  A direct `winit` dependency (the version eframe uses) forces the X11 event loop. The dev
  dependencies `egui_kittest` (`wgpu`, `eframe`) and `image` (`png`) serve only the ignored
  snapshot test.

## Public surface

- The `tbd-subtitles` binary: `tbd-subtitles [COMMAND]`, with the subcommands `gui [VIDEOS]...`,
  `process <VIDEOS>... [OPTIONS]` and `worker <STEP> <JOB_DIR>`, and `--help` and `--version`. It
  exits 0 on success, 1 with the error chain on stderr, and 2 on a usage error.
  `src/cli/README.md` describes each subcommand. There is no library target.

## Boundaries

- Depends on: `crates/pipeline/` (`run_job`, `JobOptions`, `workers::Binaries`, `tasks`,
  `graph`, `work_dir`, `progress`); `crates/job_model/` for `StepName` and the job settings;
  `crates/stages/` for the built-in One Piece glossary; the `anyhow`, `clap`, `eframe`, `tracing`
  and `tracing-subscriber` crates; at run time, `tbd-subtitles-ggml` beside it.
- Used by: people at a desktop or a terminal; the job runner in `crates/pipeline/` starts its
  `worker` subcommand; no crate links it.
- Rules:
  - the crate is the top product layer, and no crate depends on it (`cargo gates crate-layering`);
  - the source layout, the dependency directions and the file-size limits hold under
    `src/tests/architecture_rules.rs`, whose `source_inspection.rs` reads grouped imports and
    aliases and ignores comments and string literals; `cargo gates file-length` covers `src/` too;
  - `worker` never runs a Whisper step, which belongs to `tbd-subtitles-ggml`
    (`worker_takes_main_binary_steps_only` in `src/cli/tests/cli.rs`);
  - a command that cannot do its work exits non-zero and never reports success
    (`process_refuses_a_missing_video_by_name`, `a_worker_without_a_job_fails`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — the steps a `process` run goes through.
- [Desktop GUI](/documentation/features/gui.md) — the queue, progress, report, review and settings
  the window is built to show.
- [System overview](/documentation/architecture/system_overview.md) — the `gui`, `process` and
  `worker` processes and the crates behind them.
- [Development environment](/documentation/runbooks/development_environment.md) — the host, the
  container and where to run GPU work.

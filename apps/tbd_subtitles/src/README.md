# TBD Subtitles source

The source tree of the `tbd-subtitles` binary: the entry point, two composition modules (the
command line and the window), one module of shared foundations, and five feature modules, each
owning one part of the window.

## Contents

```text
apps/tbd_subtitles/src/
├── application/  the eframe window: the queue state, the frame, and the actions applied after it
├── cli/          videos alone and the `gui`, `process`, `fix` and `worker` subcommands and their dispatch
├── core/         logging and the log buffer, threads' wake, the desktop portal, the shared look
├── job_queue/    the videos waiting for subtitles: toolbar, sidebar, progress, events and edits
├── job_report/   the report of a finished job: quality checks, flagged lines, the output file
├── line_review/  reviewing a flagged line: its clip, the engines' hypotheses, the chosen text
├── log_console/  the log window: every line logged while the window runs, filtered and copied
├── main.rs       the entry point: the command line and the exit code
├── settings/     models, work folder, engines, language-model backend, output format, GPU check
└── tests/        the architecture tests and the source inspection they read imports with
```

## How it works

`main.rs` hands the process arguments to `cli`, which installs logging from `core`, and turns the
result into the exit code: the one the command chose on success (0, or 2 for a `process` run
with a job that failed its quality check), 1 with the whole error chain on stderr otherwise.
`cli` parses with clap and, for `gui`, no subcommand or `process --enqueue`, claims the single
instance and opens the window through `application` or hands the videos to the open one, or runs
its own `process`, `fix` and `worker` runners, which hand the jobs, Fix It and steps to
`crates/pipeline/`.

`application` composes the features. Each feature keeps its data in `models/` and `services/`,
free of egui, and draws in `ui/` from a narrow borrowed view the application lends it each frame;
what the user does there comes back as that feature's events (`events.rs`), which the application
turns into its `Action`s and applies after the frame, so nothing changes state while a frame is
drawn. All five features are wired in this way. `core` holds what any module may use: logging
and the log buffer the log window reads, the wake threads use, the desktop portal and its colour
scheme, number formats, the toasts, and the shared look (the mockup's palettes, Adwaita Sans and
Adwaita Mono with the Phosphor icon font, the theme built from them, and the buttons, icons and
toasts every feature draws).

```text
main.rs ──▶ cli ──▶ application ──▶ job_queue, job_report, line_review, log_console, settings
             │                        (models, services, ui, events)
             └──▶ pipeline (run_job, tasks::worker_main), job_model (StepName, JobSettings),
                  stages::adjudication::glossary

any module ──▶ core (logging, log_buffer, background, portal, color_scheme, format, ui)
```

## Public surface

None: the crate is a binary, and no module is visible outside it. The `tbd-subtitles` executable
and its subcommands are described in the crate README and in `cli/README.md`.

## Boundaries

- Depends on: `crates/pipeline/` for running jobs and worker steps; `crates/job_model/` for
  `StepName` and the job settings; `crates/stages/` for the built-in glossary; `anyhow`, `clap`,
  `eframe` with `egui-phosphor` (icons) and `winit` (the X11 event loop), `ashpd` (the desktop
  portal), `tracing` and `tracing-subscriber`.
- Used by: nothing in the repository links it; people run the binary, and the job runner in
  `crates/pipeline/` starts its `worker` subcommand.
- Rules: each held by a test in `tests/architecture_rules.rs`:
  - the top level holds only `main.rs`, this README, `tests/` and the seven module folders, each
    with a `mod.rs`; every feature has `models/mod.rs`, `services/mod.rs` and `ui/mod.rs`; and
    the crate README exists (`module_roots_and_documentation_describe_the_entire_source_tree`);
  - `core` imports no feature and no composition module; no feature imports `application` or
    `cli`; no module but `application` imports a feature's `ui` from outside that feature;
    `models/` and `services/` never name egui or eframe; no source declares an inline module or
    unit test (`dependency_boundaries_and_external_test_placement_are_enforced`, whose import
    reading `source_inspection_handles_grouped_imports_aliases_and_ignored_prose` checks);
  - a production file stays under 500 lines and a test file under 1000
    (`source_files_respect_the_size_limits_without_exemptions`), which `cargo gates file-length`
    checks as well.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — what each feature of the window is for.
- [Coding standards](/documentation/standards/coding_standards.md) — the feature folder layout and
  the layering rules the tests hold.

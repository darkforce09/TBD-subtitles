# TBD Subtitles source

The source tree of the `tbd-subtitles` binary: the entry point, two composition modules (the
command line and the window), one module of shared foundations, and four feature modules, each
owning one part of the window.

## Contents

```text
apps/tbd_subtitles/src/
├── application/  the eframe window: the queue state, the frame, and the actions applied after it
├── cli/          the `gui`, `process` and `worker` subcommands and their dispatch
├── core/         logging to stderr and the shared look of the window
├── job_queue/    the videos waiting for subtitles: the queue panel, its view, events and edits
├── job_report/   the report of a finished job: quality checks, flagged lines, the output file
├── line_review/  reviewing a flagged line: its clip, the engines' hypotheses, the chosen text
├── main.rs       the entry point: logging, the command line, the exit code
├── settings/     models, work folder, engines, language-model backend, output format, GPU check
└── tests/        the architecture tests and the source inspection they read imports with
```

## How it works

`main.rs` installs logging from `core`, hands the process arguments to `cli`, and turns the
result into the exit code: 0 on success, 1 with the whole error chain on stderr otherwise. `cli`
parses with clap and opens the window through `application` for `gui` or no subcommand, or runs
its own `process` and `worker` runners.

`application` composes the features. Each feature keeps its data in `models/` and `services/`,
free of egui, and draws in `ui/` from a narrow borrowed view the application lends it each frame;
what the user does there comes back as that feature's events (`events.rs`), which the application
turns into its `Action`s and applies after the frame, so nothing changes state while a frame is
drawn. `job_queue` is wired in this way; `job_report`, `line_review` and `settings` hold only their
module headers, and their code is not written yet. `core` holds what any module may use.

```text
main.rs ──▶ cli ──▶ application ──▶ job_queue (models, services, ui, events)
             │           │
             │           └── job_report, line_review, settings: headers only
             └──▶ job_model::StageName (crates/job_model)

any module ──▶ core (logging, ui)
```

## Public surface

None: the crate is a binary, and no module is visible outside it. The `tbd-subtitles` executable
and its subcommands are described in the crate README and in `cli/README.md`.

## Boundaries

- Depends on: `crates/job_model/` for `StageName`; `anyhow`, `clap`, `eframe`, `tracing` and
  `tracing-subscriber`.
- Used by: nothing in the repository links it; people run the binary.
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

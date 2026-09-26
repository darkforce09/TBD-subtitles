# TBD-subtitles

A local desktop app, written in Rust, that makes subtitles for your videos: every spoken line and
the important sounds (SDH), timed to the word and laid out like professional subtitles. Drop in a
video, get a `.srt` or `.ass` file beside it that VLC loads automatically. Later: translated
subtitles for Japanese text shown on screen.

**Status:** scaffold. The workspace, the desktop window, every crate and module folder with its
README, and the repository checks exist; the subtitle pipeline is built next (see the
[roadmap](/documentation/roadmap.md)).

## Layout

| Path | Contents |
|---|---|
| [`CLAUDE.md`](/CLAUDE.md) | Project laws, directory atlas and environment rules for AI sessions (`AGENTS.md` links to it) |
| [`documentation/`](/documentation/README.md) | Every document: goals, decisions, roadmap, architecture, research, features, runbooks, standards |
| [`rust-toolchain.toml`](/rust-toolchain.toml) | Rust 1.95.0 for the whole workspace |
| [`Cargo.toml`](/Cargo.toml) | The Cargo workspace: the app, the library crates and the tools |
| [`apps/`](/apps/README.md) | The `tbd-subtitles` binary: desktop window, command line, GPU workers |
| [`crates/`](/crates/README.md) | Library crates: job model, child processes, media input, subtitle formats, inference, stages, job runner |
| [`tools/`](/tools/README.md) | `cargo gates`, the checks of the repository laws |

## Build and check

```bash
cargo build --workspace
cargo test --workspace
cargo gates
```

The [development environment runbook](/documentation/runbooks/development_environment.md) covers
the host, the GPU and opening the window.

## Documentation

- [Vision and goals](/documentation/vision_and_goals.md) — what the app must do, and how fast.
- [Roadmap](/documentation/roadmap.md) — milestones M0 to M4 and their checklists.
- [Pipeline](/documentation/architecture/pipeline.md) — from video to finished subtitle file.
- [Decisions](/documentation/decisions/) — what is settled and why.
- [Continue in Claude Code](/documentation/runbooks/continue_in_claude_code.md) — the prompt that
  starts the next working session.

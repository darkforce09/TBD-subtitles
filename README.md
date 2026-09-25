# TBD-subtitles

A local desktop app, written in Rust, that makes subtitles for your videos: every spoken line and
the important sounds (SDH), timed to the word and laid out like professional subtitles. Drop in a
video, get a `.srt` or `.ass` file beside it that VLC loads automatically. Later: translated
subtitles for Japanese text shown on screen.

**Status:** planning. The repository holds the goals, research, architecture and roadmap; code
starts with milestone M0.

## Layout

| Path | Contents |
|---|---|
| [`CLAUDE.md`](/CLAUDE.md) | Project laws, directory atlas and environment rules for AI sessions (`AGENTS.md` links to it) |
| [`documentation/`](/documentation/README.md) | Every document: goals, decisions, roadmap, architecture, research, features, runbooks, standards |
| [`rust-toolchain.toml`](/rust-toolchain.toml) | Rust 1.95.0 for the whole workspace |

Planned: `apps/tbd_subtitles/` (the GUI and CLI binary), `crates/` (media, pipeline, inference,
subtitle formats) and `tools/repo_gates/` — see the
[system overview](/documentation/architecture/system_overview.md).

## Documentation

- [Vision and goals](/documentation/vision_and_goals.md) — what the app must do, and how fast.
- [Roadmap](/documentation/roadmap.md) — milestones M0 to M4 and their checklists.
- [Pipeline](/documentation/architecture/pipeline.md) — from video to finished subtitle file.
- [Decisions](/documentation/decisions.md) — what is settled and why.
- [Continue in Claude Code](/documentation/runbooks/continue_in_claude_code.md) — the prompt that
  starts the next working session.

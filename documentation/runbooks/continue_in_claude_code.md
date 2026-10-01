**Status:** live

# Continue in Claude Code

How to start the next working session on TBD-subtitles in the Claude Code GUI (Claude Desktop),
with the prompt to paste. Update the prompt whenever the roadmap's next step changes.

## Prerequisites

- Claude Desktop running (host launcher "Claude (on claude-desktop)"), signed in.
- This repository at `/run/media/system/Disk_2/Projects/TBD-subtitles`.

## Steps

1. In Claude Desktop, open the Code tab and choose the folder
   `/run/media/system/Disk_2/Projects/TBD-subtitles`.

   **Expected:** the session starts in the repository and reads `CLAUDE.md`.

2. Paste the prompt below as the first message.

   **Expected:** Claude reads the documents and shows the M2 plan, with questions on clip playback
   and "open in VLC", before building anything.

## Prompt

```text
You're continuing TBD-subtitles: a Rust desktop app that makes English SDH subtitles for videos on
my PC. The repo is /run/media/system/Disk_2/Projects/TBD-subtitles. M0 (workspace, gates), M0.5
(stack spike) and the M1 pipeline are done: `tbd-subtitles process <video>` runs 18 resumable
steps (GPU steps in workers of `tbd-subtitles` and `tbd-subtitles-ggml`) and I accepted the
Dressrosa 11 pilot. Now do milestone M2, the desktop GUI.

1. Read first: CLAUDE.md (the laws are binding: Rust only, no scripts; FFmpeg/ffprobe/claude CLI
   as the only external programs; a README in every folder from the templates; Conventional
   Commits straight to main with explicit paths; no silent deferrals). Then
   documentation/roadmap.md (M1's open item and M2), documentation/features/gui.md,
   documentation/decisions/ (the 2026-09-26 entries), documentation/architecture/pipeline.md,
   documentation/architecture/system_overview.md, documentation/research/pilot_dressrosa_11.md,
   documentation/standards/coding_standards.md and
   documentation/runbooks/development_environment.md (steps 7 and 12–13: open the window, build
   both app binaries, run a job on the host).

2. What exists: the eframe window (glow renderer) with a queue panel and the feature folders
   job_queue/, job_report/, line_review/ and settings/ (models/, services/, ui/; the rules are
   held by apps/tbd_subtitles/src/tests/architecture_rules.rs). The job runner is
   `pipeline::run_job` with `Progress` events, a `job.redb` database and `report.md` per job;
   the CLI in apps/tbd_subtitles/src/cli/process_command.rs shows how it is called.

3. Environment: you run in the claude-desktop distrobox (no CUDA driver, old FFmpeg, no display).
   Build in the container; run the window and every job on the host with distrobox-host-exec.

4. M2 scope (roadmap and gui.md): the job queue (add files or folders, reorder, cancel, retry,
   one job at a time on a background thread, per-step progress and time left); the job report
   view (QC summary, flagged lines with timestamps, the subtitle path); review of flagged lines
   (see every engine's hypothesis, pick or edit the text, re-align, rewrite the file);
   settings (settings.toml: model folder, work folder, engines, language-model backend, output
   format, GPU check) that the CLI reads too; missing models listed and downloaded with progress.
   gui.md asks to open the video in VLC and to play clips through libmpv, but the app may run
   only FFmpeg, ffprobe and claude: ask me how to handle clip playback and "open in VLC" before
   building them. The M2 acceptance ends with the batch: queue Dressrosa 12–48 in the window
   and run them, each report passing QC. Also close M1's last item, the 120-minute test (ask me
   which file to use).

5. Tests for the pure logic (queue editing, progress and time left, settings parsing, review
   edits). Before every commit: cargo fmt --all --check, cargo clippy --workspace --all-targets
   -- -D warnings, cargo test --workspace, cargo gates --with-untracked. Update READMEs and docs
   in the same commit; tick roadmap boxes as work lands.

6. Show me your M2 plan first. Ask before downloading anything new or adding a crate that links a
   native library. Stop before the batch so I can try the window myself.

Never modify the videos; keep work files out of the media folder, and log every change to the
media folder in its README.md. Ignore AGENTS.md in the media folder.
```

## Verify

After the session: `git log --oneline` shows its commits, the roadmap boxes it finished are
ticked, and CLAUDE.md's "Current state" line names the next step.

## Troubleshooting

- **Claude cannot see the repository:** the container must mount `/run/media`; see the
  [development environment](/documentation/runbooks/development_environment.md#troubleshooting).
- **GPU commands fail inside the session:** they ran in the container; prefix them with
  `distrobox-host-exec`.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the milestones the prompt walks through.
- [CLAUDE.md](/CLAUDE.md) — the laws the prompt makes binding.

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

   **Expected:** Claude reads the documents, runs the checks, and proposes the M0.5 spike plan
   before measuring anything.

## Prompt

```text
You're continuing TBD-subtitles: a Rust desktop app that generates high-quality English subtitles
(every spoken line plus SDH sound cues, timed to the word, Netflix English layout) for videos on my
PC, and later translates Japanese text shown on screen. The repo is
/run/media/system/Disk_2/Projects/TBD-subtitles. The workspace, every crate and module folder with
its README, the eframe window and the `cargo gates` checks exist (milestone M0 is done).

1. Read CLAUDE.md, then documentation/README.md, vision_and_goals.md, decisions.md, roadmap.md,
   architecture/system_overview.md, architecture/pipeline.md, research/rust_ml_stack.md and
   standards/readme_standard.md. The CLAUDE.md laws are binding: Rust only (no Python, shell,
   Makefiles or Node, ever), FFmpeg/ffprobe as the only external programs the app runs, documents
   in documentation/ following documentation/standards/, a README in every folder, Conventional
   Commits straight to main, no silent deferrals.
2. You run inside the claude-desktop distrobox, which has no CUDA driver library and an old
   FFmpeg. Run anything that touches the GPU, and FFmpeg/ffprobe, on the host with
   `distrobox-host-exec` (documentation/runbooks/development_environment.md).
3. Before and after each change: cargo fmt --all --check, cargo clippy --workspace --all-targets
   -- -D warnings, cargo test --workspace, cargo gates.
4. Milestone M0.5: show me the plan first, then spike each piece of the ML stack on
   "/run/media/system/Main_storage/Media/one_pace/done/[Muhn Pace] Dressrosa 08.mp4", measure speed,
   VRAM and RAM, write a new research snapshot plus decision entries, and check the projected
   total for a 120-minute video against the performance budget. Stop and show me.
5. Then M1: build the pipeline, run the Dressrosa 08 pilot, put the subtitle file next to the
   video, and stop so I can watch it in VLC before you batch episodes 09–48.

Never modify the videos. Log every change to the media folder in its README.md.
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

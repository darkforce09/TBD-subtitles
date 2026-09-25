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
/run/media/system/Disk_2/Projects/TBD-subtitles. Milestone M0 is done: the Cargo workspace, every
crate and module folder with its README, the eframe window, and the repository gates
(`cargo gates`). No pipeline stage is built yet.

1. Read CLAUDE.md, then documentation/README.md, vision_and_goals.md, decisions.md, roadmap.md,
   architecture/system_overview.md, architecture/pipeline.md, research/rust_ml_stack.md,
   standards/readme_standard.md, standards/coding_standards.md and
   runbooks/development_environment.md. The CLAUDE.md laws are binding: Rust only (no Python,
   shell, Makefiles or Node, ever, not even for model conversion); the app runs only FFmpeg,
   ffprobe and the claude CLI as external programs; native runtimes (ort, ggml) are allowed only
   where no pure-Rust engine is competitive; a README in every folder, written from the templates
   in documentation/standards/templates/; Conventional Commits straight to main with explicit
   paths; no silent deferrals.
2. You run inside the claude-desktop distrobox, which has no CUDA driver library and an old
   FFmpeg. Run anything that touches the GPU, and FFmpeg/ffprobe, on the host with
   `distrobox-host-exec`. Measure VRAM with nvidia-smi on the host.
3. Before every commit: cargo fmt --all --check, cargo clippy --workspace --all-targets
   -- -D warnings, cargo test --workspace, and cargo gates --with-untracked.
4. Milestone M0.5 (roadmap.md): the stack spike on
   "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 11.mp4" (30.9 minutes;
   episode 11 is the pilot, see decisions.md). Show me the plan first: which roadmap item uses
   which crate and model file, where each model comes from and how big it is, which code goes into
   which crate or module folder (reusable code in the crate it belongs to, the measuring harness
   as a Rust tool under tools/), and where the CUDA 13 libraries for ort will live. Ask me before
   downloading anything. Then, for every M0.5 item, run it on episode 11 from Rust and measure
   wall time, speed against realtime, peak VRAM and peak RAM, with quality notes. Write a new
   research snapshot from the research snapshot template, decision entries that settle the open
   questions (second speech engine, default language-model backend) and each stack choice, the
   working CUDA recipe in the development environment runbook, and tick the roadmap boxes in the
   same commits. Project the total for a 120-minute video against the performance budget in
   vision_and_goals.md, and say plainly which stages miss it and what would fix them. Stop and
   show me.
5. After I approve, M1: build the pipeline, run the Dressrosa 11 pilot, put the subtitle file next
   to the video, and stop so I can watch it in VLC before you batch episodes 12–48.

Never modify the videos, and keep work files out of the media folder. Log every change to the
media folder in its README.md.

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

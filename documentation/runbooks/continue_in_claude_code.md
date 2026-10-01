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

   **Expected:** Claude reads the documents and shows the M6 plan, with the measurement each item
   is judged by, before building anything.

## Prompt

```text
You're continuing TBD-subtitles: a Rust desktop app that makes English SDH subtitles for videos on
my PC, translates the Japanese writing on screen and can replace it inside a localized copy of the
video. The repo is /run/media/system/Disk_2/Projects/TBD-subtitles. M0 to M3 are done and
accepted; M4 (on-screen text) and M5 (in-place replacement) are implemented and under validation,
not accepted. The binary storage foundation is built: each job keeps every step's output and
record in one job.redb (redb, values archived with rkyv) that one process owns, workers stream
their outputs to the runner over framed pipes, the per-frame `frames` and `readings` tables hold
each sign's frames and read-back results, and approved signs are shared between episodes in
library.redb. `tbd-subtitles process <video>` runs 29 resumable steps in `tbd-subtitles`,
`tbd-subtitles-ggml` and `tbd-subtitles-llm`. Now do milestone M6, the 24 GB workstation scaling.

1. Read first: CLAUDE.md (the laws are binding: Rust only, no scripts; FFmpeg/ffprobe/claude CLI
   as the only external programs; peak RAM within 24 GB and 6.5 GB of VRAM per GPU worker; a
   README in every folder from the templates; Conventional Commits straight to main with explicit
   paths; no silent deferrals). Then documentation/roadmap.md (M4 to M6),
   documentation/optimizations/memory_profiles.md, documentation/decisions/foundations.md (the
   24 GB entry) and documentation/decisions/storage.md,
   documentation/architecture/system_overview.md, documentation/architecture/pipeline.md,
   documentation/architecture/binary_storage_plan.md,
   documentation/research/visual_scan_dressrosa_11.md,
   documentation/research/per_frame_tables.md, documentation/standards/coding_standards.md and
   documentation/runbooks/development_environment.md (steps 12 and 13: build the three app
   binaries, run a job on the host, print its rows with `tbd-subtitles dump`).

2. Environment: you run in the claude-desktop distrobox (no CUDA driver, old FFmpeg, no display).
   Build in the container; run the window and every job on the host with distrobox-host-exec.

3. M6 scope (roadmap): native 1080p visual screening across the CPU's threads; audio
   recognition and visual screening at the same time; bounded in-memory frame ring buffers for
   decoding and encoding; a resident plate, mask and patch cache for the localized video; one
   spectrogram cache shared by the audio steps; a larger local translation model. Each use of the
   memory headroom is measured on a real episode before it is kept (the 24 GB decision), and
   Dressrosa 11 and 28 keep identical subtitle files and the same approved replacements unless a
   change is meant to alter them.

4. Before every commit: cargo fmt --all --check, cargo clippy --workspace --all-targets
   -- -D warnings, cargo test --workspace, cargo gates --with-untracked. Update READMEs and docs
   in the same commit; tick roadmap boxes as work lands; rebuild the AppImage (cargo appimage)
   after any change I should see in the app.

5. Show me your M6 plan first, with the measurement each item is judged by. Ask before
   downloading anything new (a larger Qwen model among them) or adding a crate that links a
   native library.

Never modify the videos; keep work files out of the media folder, and log every change to the
media folder in its README.md. Ignore AGENTS.md in the media folder.
```

## Verify

After the session: `git log --oneline` shows its commits, the roadmap boxes it finished are
ticked, and CLAUDE.md's "Current state" paragraph names the next step.

## Troubleshooting

- **Claude cannot see the repository:** the container must mount `/run/media`; see the
  [development environment](/documentation/runbooks/development_environment.md#troubleshooting).
- **GPU commands fail inside the session:** they ran in the container; prefix them with
  `distrobox-host-exec`.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the milestones the prompt walks through.
- [CLAUDE.md](/CLAUDE.md) — the laws the prompt makes binding.

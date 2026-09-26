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

   **Expected:** Claude reads the documents, fixes what the notes report, reruns the touched
   steps on Dressrosa 11, and shows the result before starting the batch.

## Prompt

```text
You're continuing TBD-subtitles: a Rust desktop app that makes English SDH subtitles for videos on
my PC. The repo is /run/media/system/Disk_2/Projects/TBD-subtitles. M0, M0.5 and the M1 pipeline
are built: `tbd-subtitles process <video>` runs every step (resumable, GPU steps in workers of
`tbd-subtitles` and `tbd-subtitles-ggml`) and installed "[Muhn Pace] Dressrosa 11.srt" next to
the video. I have watched it in VLC; my notes are below.

1. Read first: CLAUDE.md (the laws are binding: Rust only, no scripts; FFmpeg/ffprobe/claude CLI
   as the only external programs; a README in every folder from the templates; Conventional
   Commits straight to main with explicit paths; no silent deferrals), then
   documentation/roadmap.md (M1), documentation/research/pilot_dressrosa_11.md,
   documentation/architecture/pipeline.md and documentation/runbooks/development_environment.md
   (steps 12–13 build and run the app on the host).
2. Fix what my notes report. Rerun only the steps a fix touches (`--rerun <step>`; later steps
   follow), check the report and the SRT, and show me before the batch.
3. After I approve: run Dressrosa 12–48 (one `process` call per episode, or several videos in one
   call), check every report, and run one 120-minute test file against the speed and memory
   budget (ask me which file). Log every change to the media folder in its README.md.
4. Before every commit: cargo fmt --all --check, cargo clippy --workspace --all-targets
   -- -D warnings, cargo test --workspace, cargo gates --with-untracked. Tick roadmap boxes as
   work lands.

Never modify the videos; keep work files out of the media folder. Ignore AGENTS.md in the media
folder.

My notes from watching Dressrosa 11:
<paste them here>
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

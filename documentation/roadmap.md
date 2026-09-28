**Status:** live

# Roadmap

The milestones from an empty repository to a finished app, each with a checklist and an acceptance
test. Tick items in the same commit as the work. A milestone is done when its acceptance test
passes; nothing moves to a later milestone without the owner saying so.

## M0 — Workspace skeleton and gates

- [x] Root `Cargo.toml` workspace (resolver 3, members only); every crate sets edition 2024,
      `rust-version = "1.95"`, `publish = false`.
- [x] `apps/tbd_subtitles`: one binary with clap subcommands (`gui`, `process`, `worker <stage>`),
      an eframe window that opens, and a README.md.
- [x] Crate skeletons from the [system overview](/documentation/architecture/system_overview.md),
      each with a README.md and a `//!` module header, and a README.md in every module folder.
- [x] `crates/child_process`: child processes with deadlines, process-group kills and drained
      pipes, for FFmpeg, ffprobe, the `claude` CLI and the GPU workers.
- [x] `tools/repo_gates` (`cargo gates`) on `tools/verification_core`: fails on tracked Python,
      shell, Makefile or Node files, production files of 500 lines or more, test files of 1000 or
      more, missing module headers, inline tests, ticket and milestone ids and history words,
      whitespace errors, upward crate dependencies, folders without a README.md or with a
      Contents block that does not match, READMEs out of shape, documents without a status line,
      and broken links, anchors, backticked paths and cited commands.
- [x] Architecture rule tests: lower layers never import higher ones (`cargo gates
      crate-layering`, and the app's feature-folder tests).
- [x] README standard and templates for every README kind and document type.

**Acceptance:** `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
`cargo test --workspace` and `cargo gates` all pass; `tbd-subtitles gui` opens a window on the
host.

## M0.5 — Stack spike on Dressrosa 11

Prove each piece of the [Rust ML stack](/documentation/research/rust_ml_stack.md) on real audio
before building the pipeline around it. Record speed, VRAM, RAM and quality notes in a new research
snapshot, and settle the matching open questions in [decisions](/documentation/decisions/).

- [x] FFmpeg streaming: 16 kHz mono f32 through a pipe in bounded chunks; ffprobe JSON for tracks.
- [x] Vocal separation: MDX-Net Voc_FT vs Mel-RoFormer ONNX on CUDA — speed per hour of audio.
- [x] Voice activity detection: earshot on the vocal stem.
- [x] Speech recognition: parakeet-rs with Parakeet-TDT-0.6B-v2, and one second engine.
- [x] Forced alignment: CTC Viterbi over parakeet-ctc ONNX vs the Qwen3 aligner.
- [x] Sound events: CED-base on both stems, through our own ONNX runner (the soundevents crate
      forces OpenSSL into the build).
- [x] Language model: `claude -p` with a JSON schema vs mistral.rs with a 4B model.
- [x] CUDA libraries for `ort` on Bazzite (the user runtime folder, or beside the binary) —
      working recipe written into the [development environment](/documentation/runbooks/development_environment.md) runbook.

Results: [stack spike on Dressrosa 11](/documentation/research/stack_spike_dressrosa_11.md).

**Acceptance:** every item runs on Dressrosa 11 from Rust with measured numbers, and the projected
total for a 120-minute video is within the [performance budget](/documentation/vision_and_goals.md#performance-budget).

## M1 — Pipeline and the Dressrosa pilot

- [x] Every stage of the [pipeline](/documentation/architecture/pipeline.md) as a resumable stage
      with typed JSON output; `tbd-subtitles process <video>` runs them in order.
- [x] QC report per job (layout checks, uncovered speech, flagged lines with timestamps).
- [x] Pilot run: Dressrosa 11 → subtitle file installed next to the video
      ([pilot run](/documentation/research/pilot_dressrosa_11.md)).
- [x] Pilot review: the owner watched it in VLC and accepted it (2026-09-26).
- [x] A 120-minute test file within the speed and memory budget
      ([120-minute test](/documentation/research/long_video_120min.md)).
- [x] Log the run in the media folder's README.md.

**Acceptance:** the owner accepts the pilot; a 120-minute test file meets the speed and memory
budget. The batch of Dressrosa 12–48 moved to M2 at the owner's word: it runs from the GUI.

## M2 — Desktop GUI

- [x] Job queue: add files or folders, reorder, cancel, retry; per-stage progress and time left.
- [x] Job report view: QC results and the list of flagged lines.
- [x] Review flagged lines: play the clip, pick or edit the text, re-align, save.
- [x] Settings: model folder, engines, output format, language-model backend, GPU checks.
- [x] Redesign: the owner's approved macOS-like window, built in eight phases: the look (the
  mockup's palettes, Adwaita Sans, Phosphor icons, the desktop's light or dark, X11), the queue's
  states, toolbar and sidebar, the detail pane, the overview, Check Lines and the Settings window.
- [ ] Batch: Dressrosa 12–48 queued and run from the window, each with a report that passes QC.
- [ ] Packaging: `cargo appimage` builds one self-contained AppImage (bundled CUDA, cuDNN, ONNX
  Runtime and FFmpeg); host verification of the built image is still pending.

Details: [GUI](/documentation/features/gui.md), [building the
AppImage](/documentation/runbooks/building_the_appimage.md).

**Acceptance:** the owner processes a new video from the GUI alone and fixes a flagged line in it;
episodes 12–48 have subtitles, made from the GUI, that pass QC.

## M3 — Automation

- [ ] Watch folders: new videos are queued once they finish downloading.
- [ ] Dolphin right-click entry "Generate subtitles" that queues the selected videos.
- [ ] Single-instance hand-off: a second launch passes its files to the running app.
- [ ] Desktop entry and a notification when a job finishes.

Details: [automation](/documentation/features/automation.md).

**Acceptance:** a video copied into a watched folder gets subtitles with no further action.

## M4 — Japanese on-screen text

- [ ] Confirm with the owner what "Japanese text" covers (on-screen writing, and whether Japanese
      speech or songs count too).
- [ ] Source A: re-time the One Pace sign translations to the dub edit.
- [ ] Source B: detect, read and translate on-screen Japanese text; positioned ASS output.

Details: [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md).

**Acceptance:** Dressrosa signs and title cards show translated, positioned subtitles in VLC.

## Later

Items the owner moved out of the pipeline milestone to keep it small (see the
[decisions](/documentation/decisions/)); each waits for the owner to place it in a milestone.

- [ ] Silero for voice-activity frames near earshot's threshold.
- [ ] CLAP zero-shot sound classes for sounds AudioSet lacks.
- [ ] Reference subtitles (the One Pace `.ass` files) as meaning and spelling hints for the
      language model.
- [ ] Speaker labels (`[Law]`) for voices the language model judges off-screen.
- [ ] The local language model (mistral.rs) as an app worker binary, the offline fallback to
      `claude -p`.

## Open questions

| Question | Settle by |
|---|---|
| Output format when sign subtitles exist: always `.ass`, or `.srt` until signs appear? | M4 |
| Which Japanese text counts: on-screen only, or also Japanese speech and songs? | Owner, at M4 start |

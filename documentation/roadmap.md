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

## M0.5 — Stack spike on Dressrosa 08

Prove each piece of the [Rust ML stack](/documentation/research/rust_ml_stack.md) on real audio
before building the pipeline around it. Record speed, VRAM, RAM and quality notes in a new research
snapshot, and settle the matching open questions in [decisions](/documentation/decisions.md).

- [ ] FFmpeg streaming: 16 kHz mono f32 through a pipe in bounded chunks; ffprobe JSON for tracks.
- [ ] Vocal separation: MDX-Net Voc_FT vs Mel-RoFormer ONNX on CUDA — speed per hour of audio.
- [ ] Voice activity detection: earshot on the vocal stem.
- [ ] Speech recognition: parakeet-rs with Parakeet-TDT-0.6B-v2, and one second engine.
- [ ] Forced alignment: CTC Viterbi over parakeet-ctc ONNX vs the Qwen3 aligner.
- [ ] Sound events: CED through the soundevents crate on the background stem.
- [ ] Language model: `claude -p` with a JSON schema vs mistral.rs with a 4B model.
- [ ] CUDA libraries for `ort` on Bazzite (shipped beside the binary) — working recipe written
      into the [development environment](/documentation/runbooks/development_environment.md) runbook.

**Acceptance:** every item runs on Dressrosa 08 from Rust with measured numbers, and the projected
total for a 120-minute video is within the [performance budget](/documentation/vision_and_goals.md#performance-budget).

## M1 — Pipeline and the Dressrosa pilot

- [ ] Every stage of the [pipeline](/documentation/architecture/pipeline.md) as a resumable stage
      with typed JSON output; `tbd-subtitles process <video>` runs them in order.
- [ ] QC report per job (layout checks, uncovered speech, flagged lines with timestamps).
- [ ] Pilot: Dressrosa 08 → subtitle file installed next to the video → **stop; the owner watches
      it in VLC and reports problems** → fixes.
- [ ] Batch: Dressrosa 09–48 after the owner approves the pilot.
- [ ] Log the run in the media folder's README.md.

**Acceptance:** the owner accepts the pilot; all 41 episodes have subtitles that pass QC; a
120-minute test file meets the speed and memory budget.

## M2 — Desktop GUI

- [ ] Job queue: add files or folders, reorder, cancel, retry; per-stage progress and time left.
- [ ] Job report view: QC results and the list of flagged lines.
- [ ] Review flagged lines: play the clip, pick or edit the text, re-align, save.
- [ ] Settings: model folder, engines, output format, language-model backend, GPU checks.

Details: [GUI](/documentation/features/gui.md).

**Acceptance:** the owner processes a new video from the GUI alone and fixes a flagged line in it.

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

## Open questions

| Question | Settle by |
|---|---|
| Second speech engine: Whisper large-v3 (whisper-rs), Canary or Granite (transcribe-cpp/crispasr), or Kyutai 1B (candle)? | M0.5 measurements |
| Default language-model backend: `claude -p` (owner's subscription) or local mistral.rs? | M0.5 quality check |
| Output format when sign subtitles exist: always `.ass`, or `.srt` until signs appear? | M4 |
| Where models live and how the first download is shown to the user | M2 |
| Which Japanese text counts: on-screen only, or also Japanese speech and songs? | Owner, at M4 start |

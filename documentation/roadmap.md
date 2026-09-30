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
- [x] Batch: Dressrosa 12–48 queued and run from the window: 38 videos with subtitles, accepted by
  the owner on 2026-09-29. After Fix It, nine reports flag one problem each: a too-short cue in
  14, 20, 24, 25, 27, 28 and 29, speech with no cue in 25, 31 and 35.
- [x] Packaging: `cargo appimage` builds one self-contained AppImage (bundled CUDA, cuDNN, ONNX
  Runtime and FFmpeg); it runs a full job on the host with only the NVIDIA driver, and opens
  from Gear Lever.
- [x] Fix It: on a finished job, a stronger `claude` model (Opus by default, chosen in Settings,
  Engines) reads the whole video, fixes its flagged lines and checks each change; the owner keeps
  or undoes each in Check Lines.

Details: [GUI](/documentation/features/gui.md), [Fix It](/documentation/features/fix_it.md),
[building the AppImage](/documentation/runbooks/building_the_appimage.md).

**Acceptance:** the owner processes a new video from the GUI alone and fixes a flagged line in it;
episodes 12–48 have subtitles, made from the GUI, that pass QC. M2 is done at the owner's word: the
owner accepted the batch on 2026-09-29.

## M3 — Automation

- [x] Watch folders: new videos are queued once they finish downloading.
- [x] Dolphin right-click entry "Generate subtitles" that queues the selected videos.
- [x] Single-instance hand-off: a second launch passes its files to the running app.
- [x] Desktop entry and a notification when a job finishes.

Built: watch folders in Settings, Automation, scanned with their subfolders every 15 seconds
while the app is open, queuing each finished video without subtitles once and starting the
queue; the Dolphin entry, written by the app when it starts from its AppImage; one window per
session, which later launches hand their videos to over a local socket; the desktop entry with
`%F` for "Open With"; a notification when a job ends while the window is away; the window's
icon; and a queue that keeps running while the window is minimized. The owner tested every item
on the host with the AppImage and accepted M3 on 2026-09-29.

Details: [automation](/documentation/features/automation.md).

**Acceptance:** a video copied into a watched folder gets subtitles with no further action while
the app is open.

## M4 — Japanese on-screen text

- [x] Implement translation for visible Japanese, including credits, decorative writing and visible lyrics.
- [x] Six resumable visual steps in the normal job: detect, read, track, translate, review and typeset.
- [x] Local PP-OCRv5 and manga-ocr; isolated Qwen worker; optional tool-disabled Claude image fallback.
- [x] Conservative scene-validated reference wording; source geometry and timing are recalculated.
- [x] Combined ASS with separate visual layout, flagged nearby fallbacks and source videos preserved.
- [x] Settings, combined queue/progress, Overview counts and Check Text with actual ASS comparison,
      playback, frame stepping and keep/undo/reprocess corrections.
- [ ] Annotated board, title and name-card pilots and Dressrosa 11, 16 and 39 scene acceptance.
- [x] One full episode (roughly 20–30 minutes): measured visual time and 8 GB RAM / 5.5 GB VRAM limits
      ([Dressrosa 11 visual scan](/documentation/research/visual_scan_dressrosa_11.md): 3.8–4.0 minutes of
      visual processing, detection at 206–214 frames per second, 3.8 GB RAM and 1.9 GB VRAM at most).
- [ ] Final repository checks, AppImage rebuild and host smoke test.
- [ ] Owner acceptance of the complete GUI correction flow and VLC playback.

Details: [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md).

**Acceptance:** every readable annotated occurrence is translated or explicitly flagged; timing is
within one source frame and accepted tracks stay within two pixels at 1080p. The owner accepts
translated Dressrosa signs in VLC and the complete desktop review workflow. The owner selects a
single full episode for the memory/time benchmark; a two-hour visual benchmark is not required.
Missed faint text and false detections are accepted limitations, as is writing shorter than the
half-second sample step that no sample or cut lands on. The sampled scan with bisected boundaries
and one Claude call per keyframe is built; its single-episode measurement is the open benchmark.

## M5 — In-place on-screen text

Replace visible Japanese inside the picture, as Google Translate does, in a localized copy of the
video beside the source. M4 stays open as it stands; M5 builds on its detection and translation.

- [x] Three resumable replacement steps between review and typesetting: stroke masks with
      per-frame following of moving writing, LaMa inpainting through ONNX Runtime in its own
      worker, and Noto Sans lettering in the measured style through tiny-skia.
- [x] A `localized_video` step after the output: every frame decoded, the patches blended and the
      video re-encoded with `hevc_nvenc` (libx264 fallback), its peak rate capped near the
      source's, audio and chapters copied, no subtitle stream: `<video>.localized.mkv`.
- [x] `<video>.localized.ass` with the dialogue and sound cues alone, no on-screen text events,
      each cue moved to the top while English drawn into the video sits under it; each fallback
      keeps its reason in Check Text; `<video>.ass` unchanged.
- [x] Guards: variable frame rate refused, a `.localized.mkv` the job did not write never
      overwritten, watch folders and folder adds skip `*.localized.mkv`.
- [x] Settings → On-screen Text "Replace text in the video", on by default for new jobs; the
      `lama-inpaint` and `latin-fonts` models in the model manifest and the download list.
- [x] Check Text's Subtitles | Localized video control, Show erase mask and replacement status;
      the Overview's localized-video card and replaced count; the job-end notification's line.
- [x] One episode measured: Dressrosa 11, 4.9 minutes added, 707 MB against a 647 MB source,
      15 of 21 candidates replaced
      ([localized video on Dressrosa 11](/documentation/research/localized_video_dressrosa_11.md)).
- [x] Outlined lettering on translucent name cards, numerals on signs and strokes a detector box
      clips separate cleanly: the Rebecca name and role cards are replaced on Dressrosa 11.
- [ ] Playback check of the localized video with its `.localized.ass` in VLC and mpv.
- [ ] AppImage rebuild and host smoke test with the localized video on.
- [ ] Owner acceptance of the localized video and its review in Check Text.

Details: [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md#replacement-in-the-video),
[video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).

**Acceptance:** the owner watches a localized Dressrosa episode with its `.localized.ass` in VLC
and accepts it, and accepts the localized video's review in Check Text.

## M6 — 24 GB workstation scaling (DDR5-6000 high-throughput architecture)

Following the completion of the redb and rkyv binary storage foundation, scale directly to the
owner's 32 GB DDR5-6000 workstation hardware (24 GB RAM target, 5.5 GB worker VRAM boundary),
bypassing throwaway intermediate 8 GB/16 GB redesigns.

- [ ] Full 1080p native visual screening across 28 threads (no 640-pixel proxy information loss).
- [ ] Concurrent audio recognition and visual screening execution (35–45% total wall-time reduction).
- [ ] In-memory uncompressed video frame ring buffers (3–4 GB) for zero-stall decoding and encoding.
- [ ] 6–8 GB resident plate, mask, and composed patch cache (zero disk reads during localization).
- [ ] Resident in-memory Mel spectrogram tensor cache shared across VAD, CED, alignment, and ASR.
- [ ] Upgraded local translation models (Qwen 7B / 14B) for near-human Japanese idiom translation.

Details: [Memory profiles](/documentation/optimizations/memory_profiles.md).

**Acceptance:** Dressrosa 11 processes end-to-end in under 4 minutes wall time with zero disk
thrashing; visual text screening runs at native 1080p; audio and visual screening run concurrently.

## M7 — High-accuracy dialogue and audio ensembling

Advance dialogue accuracy from 95% to 99%+ by closing the diff-sheet backbone blind spot,
biasing Whisper toward arc vocabulary, enabling self-learning series glossaries, and utilizing
the 24 GB memory headroom for full-bandwidth separation.

- [ ] Full-bandwidth 44.1 kHz / 48 kHz stereo vocal separation preserving high-frequency consonant transients.
- [ ] Multi-model separation ensemble (Mel-Band RoFormer + HTDemucs v4) eliminating vocal dropouts.
- [ ] Symmetric orphan recovery in `diff_sheet`: preserve speech heard by secondary engines when
      the backbone chunk has zero words.
- [ ] Dynamic arc vocabulary prompt in Whisper (`initial_prompt` via CrispASR) to eliminate
      phonetic English drift on character names, attacks, and locations.
- [ ] Self-learning series dictionary in `library.redb`: confirmed name corrections in Check Lines
      automatically propagate to subsequent queued episodes.
- [ ] Conversational dialogue context windows in Claude adjudication (feeding the previous three
      settled utterances).
- [ ] Selective vocal stem normalization and consonant pre-emphasis for `UNSURE` re-decodes.
- [ ] Third ASR engine acoustic voting (Qwen3-ASR or Canary) to break 1-vs-1 engine ties.
- [ ] Global episode-wide acoustic memory for automated speaker diarization and character attribution.

Details: [Audio accuracy](/documentation/optimizations/audio_accuracy.md).

**Acceptance:** zero spoken lines dropped across an entire episode; proper-noun errors reduced to
under one per episode on Dressrosa benchmark episodes; speech recognition exhibits zero consonant clipping.

## M8 — Advanced visual tracking and video acceleration

Achieve sub-pixel perspective stability on moving signs, eliminate inpainting flicker, and
accelerate `localized_video` from minutes to seconds.

- [ ] Planar homography and optical flow tracking: compute 3×3 perspective transformation matrices
      per frame and store them in `job.redb`.
- [ ] Motion-compensated plate warping: inpaint primary keyframes with LaMa and warp plates along
      tracking vectors to eliminate background flicker.
- [ ] Smart lossless segment re-encoding: cut video at GOP boundaries, re-encode only intervals
      with active patches, and concatenate untouched footage losslessly (`localized_video` under 15 seconds).
- [ ] Trajectory smoothing (Kalman filtering) and shot-boundary snapping for entrance/exit stability.
- [ ] Multi-modal audio-visual synchronization: snap on-screen text appearances to sharp sound cues.

Details: [Visual and video](/documentation/optimizations/visual_and_video.md).

**Acceptance:** localized video encoding for a 30-minute episode finishes in under 20 seconds; moving
text replacements exhibit zero jitter and zero background flicker.

## Later

Items the owner moved out of the pipeline milestone to keep it small (see the
[decisions](/documentation/decisions/)); each waits for the owner to place it in a milestone.

- [ ] Silero for voice-activity frames near earshot's threshold.
- [ ] CLAP zero-shot sound classes for sounds AudioSet lacks.
- [ ] Reference subtitles (the One Pace `.ass` files) as meaning and spelling hints for the
      language model.
- [ ] Speaker labels (`[Law]`) for voices the language model judges off-screen.
- [ ] The local language model as an offline fallback for audio adjudication. The isolated
      mistral.rs worker supplies visual translation.

## Open questions

| Question | Settle by |
|---|---|
| Visual output | ASS whenever on-screen translation is enabled; with Replace text in the video on (the default), also a localized video and its own ASS ([decision](/documentation/decisions/stack_and_pipeline.md#2026-09-30--writing-is-replaced-in-a-localized-video-re-encoded-beside-the-source)) |
| Visible Japanese scope | All readable writing, including credits, decoration and visible lyrics |

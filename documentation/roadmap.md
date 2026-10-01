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
      with typed output; `tbd-subtitles process <video>` runs them in order.
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
- [x] Local PP-OCRv5 and manga-ocr; tool-disabled Claude first, one call per keyframe; isolated Qwen worker for the rest.
- [x] Conservative scene-validated reference wording; source geometry and timing are recalculated.
- [x] Combined ASS with separate visual layout, flagged nearby fallbacks and source videos preserved.
- [x] Settings, combined queue/progress, Overview counts and Check Text with actual ASS comparison,
      playback, frame stepping and keep/undo/reprocess corrections.
- [ ] Annotated board, title and name-card pilots and Dressrosa 11, 16 and 39 scene acceptance.
- [x] One full episode (roughly 20–30 minutes): measured visual time, RAM and VRAM
      ([Dressrosa 11 visual scan](/documentation/research/visual_scan_dressrosa_11.md): 3.8–4.0 minutes of
      visual processing, detection at 206–214 frames per second, 3.8 GB RAM and 3.6 GB VRAM at most).
- [x] Final repository checks, AppImage rebuild and host smoke test: the AppImage runs the visual
      steps on the host ([localized video polish](/documentation/research/localized_video_polish_dressrosa_28.md)).
- [ ] Owner acceptance of the complete GUI correction flow and VLC playback.

Details: [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md).

**Acceptance:** every readable annotated occurrence is translated or explicitly flagged; timing is
within one source frame and accepted tracks stay within two pixels at 1080p. The owner accepts
translated Dressrosa signs in VLC and the complete desktop review workflow. The owner selects a
single full episode for the memory/time benchmark; a two-hour visual benchmark is not required.
Missed faint text and false detections are accepted limitations, as is writing shorter than the
half-second sample step that no sample or cut lands on. The sampled scan with bisected boundaries
and one Claude call per keyframe is built; on Dressrosa 11 its detection comes to 3.9 minutes per
50,000 frames, within the six-minute target.

## M5 — In-place on-screen text

Replace visible Japanese inside the picture, as Google Translate does, in a localized copy of the
video beside the source. M4 stays open as it stands; M5 builds on its detection and translation.

- [x] Three resumable replacement steps between review and typesetting: stroke masks with
      per-frame following of moving writing, LaMa inpainting through ONNX Runtime in its own
      worker, and Noto Sans lettering in the measured style through tiny-skia.
- [x] `text_verify`: sampled finished frames rebuilt and read back by the local PP-OCRv5; only a
      replacement with no Japanese left and English that reads back stays in the video.
- [x] One sign is one occurrence with its furigana, erased with its line and never lettered alone.
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
- [x] One episode measured: Dressrosa 11, 4.9 minutes added, 707 MB against a 647 MB source
      ([localized video on Dressrosa 11](/documentation/research/localized_video_dressrosa_11.md));
      with the read-back check, 7 of its 10 candidates are replaced, and 25 on Dressrosa 28
      ([localized video polish](/documentation/research/localized_video_polish_dressrosa_28.md)).
- [x] Outlined lettering on translucent name cards, numerals on signs and strokes a detector box
      clips separate cleanly: the Rebecca name and role cards are replaced on Dressrosa 11.
- [x] AppImage rebuild and host run with the localized video on: Dressrosa 11 and 28 from the
      AppImage on the host ([localized video polish](/documentation/research/localized_video_polish_dressrosa_28.md)).
- [ ] The open issues of the [polish record](/documentation/research/localized_video_polish_dressrosa_28.md#open-issues):
      the 海 wall on Dressrosa 11 no longer replaced (its joined pieces cannot be followed), 2段目
      never detected, faint outlines of erased strokes, leftover furigana that block a
      replacement, and huge moving writing that can time out.
- [ ] Playback check of the localized video with its `.localized.ass` in VLC and mpv.
- [ ] Owner acceptance of the localized video and its review in Check Text.

Details: [Japanese on-screen text](/documentation/features/japanese_onscreen_text.md#replacement-in-the-video),
[video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).

**Acceptance:** the owner watches a localized Dressrosa episode with its `.localized.ass` in VLC
and accepts it, and accepts the localized video's review in Check Text.

## M6 — 24 GB workstation throughput

Use the owner's machine (i7-14700K, 32 GB DDR5-6000, RTX 3070) directly: 24 GB of RAM at peak
([decision](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram))
and 6.5 GB of VRAM per GPU worker, each GPU step waiting, up to a deadline, for the memory it
measured
([decision](/documentation/decisions/foundations.md#2026-10-01--each-gpu-worker-stays-within-65-gb-of-vram-and-a-step-waits-up-to-a-deadline-for-the-memory-it-measured)).
The GPU lock stays: two GPU steps never share the card, so the headroom helps where frames are
held or a step waits on something other than the GPU. The owner picks which items to build.

- [x] Binary storage: `job.redb` per job (step outputs and records, the worker channel, the
      per-frame `frames` and `readings` tables) and the `library.redb` sign library, all six
      phases ([binary storage plan](/documentation/architecture/binary_storage_plan.md)).
- [x] Baseline: Dressrosa 11 and 28 from scratch with the current build on the host; each step's
      wall time, real-time factor, CPU cores, GPU, NVENC and NVDEC use, peak RAM, whole-job RAM and
      VRAM, the localized video's decode, blend and encode time and the screen's phases; 16.4 and
      15.1 minutes of real wall time, whole-job peak RAM 3.9 GB
      ([M6 baseline](/documentation/research/m6_baseline.md)). Every later target in M6, M7 and
      M8 is stated against it; an item is compared by resuming a copy of the baseline job from
      the first step it changes, since Claude's answers differ between runs from scratch.
- [x] Separation within the VRAM cap: every ONNX Runtime CUDA session caps its arena at 4.5 GiB;
      the separation worker went from 7.3 GB to 4.2 GB with byte-identical stems and the same
      wall time ([measurement](/documentation/research/m6_separation_limit_and_overlap.md)).
- [ ] The VRAM cap at 6.5 GB per GPU worker: after taking the GPU lock, each GPU step waits until
      the free memory covers its measured need, saying how much is free and needed, and fails
      after ten minutes with a message to close other GPU programs. Built, awaiting the host
      measurement in the [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] Full-resolution visual screening: samples and bisection probes screened at the source's
      resolution as 8-bit YUV, converted in Rust and padded to 1,088 lines, on two detector
      sessions in one worker, batch and pool one measured pair; duplicates and signatures read
      from luma; each occurrence confirmed by the server detector at full resolution on the
      screened sample nearest its middle, held in RAM within 4 GiB
      ([decision](/documentation/decisions/onscreen_detection.md#2026-10-01--the-detector-screens-full-resolution-frames-padded-to-a-multiple-of-32-on-two-sessions)).
      Built, awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] The TensorRT detector engine: Settings, On-screen Text, "Detector engine: TensorRT / CUDA"
      for the screening and confirmation sessions; FP16 engines built inside the runtime from the
      exported ONNX files and cached by GPU, driver, TensorRT version, model and input shape;
      TensorRT bundled in the AppImage. The default stays CUDA until the host bench confirms
      TensorRT
      ([decision](/documentation/decisions/inference_engines.md#2026-10-01--tensorrt-runs-the-pp-ocrv5-detectors),
      [conversion](/documentation/decisions/inference_engines.md#2026-10-01--models-may-be-converted-or-compiled-when-a-measurement-shows-it-pays)).
      Built, awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] Hardware decoding (NVDEC): an optional CPU or GPU decode setting for the screen, Settings,
      On-screen Text, "Decode video on the GPU (NVDEC)", CPU by default and outside every
      fingerprint; on the RTX 3070 the detect-bench measured NVDEC at about 770 frames per second
      against about 1,500 for the CPU's `yuv420p`. Built, awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] YUV region decoding for `text_mask`, `text_inpaint` and `text_verify`: FFmpeg delivers
      `yuv420p` cropped to the even-aligned rectangle around the region, converted in Rust with
      the stream's matrix and range and decoded ahead on a thread through a bounded queue. Built,
      awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] Bounded frame queues in the localized video: a decode thread, the blend on the step's
      thread and an encoder thread, joined by bounded queues of about five seconds of frames.
      Built, awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] Audio steps first at the GPU lock, and a late lock for `text_translate`: a visual step stays
      off the lock while an audio step waits for it; `text_translate` takes the lock and the
      memory wait only when the local model loads, and holds them until the model is dropped.
      Built, awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md), which also
      measures whether `text_detect` now ends before adjudication.
- [x] Overlapping steps that wait on different things: the screen, reading and tracking (which
      need only the probe and the shot scan) run as a lane beside adjudication; 111–139 s saved
      per episode, 12–15 % of the whole job, limited by detection outlasting adjudication
      ([measurement](/documentation/research/m6_separation_limit_and_overlap.md)).
- [ ] A larger local translation model: a 7B-class model at 4-bit (about 4.5 GB, within the VRAM
      cap) as the trial; a 14B model (about 8–9 GB at 4-bit) only with CPU offload, and only if
      the 7B trial shows the gain.

Details: [memory profiles](/documentation/optimizations/memory_profiles.md).

**Acceptance:** the baseline is recorded. Each item built is measured against it on Dressrosa 11
and 28 in a research snapshot: per-step and whole-job wall time, peak RAM within 24 GB, every GPU
worker within 6.5 GB of VRAM, and the same subtitle files and approved replacements unless the item
means to change them. The owner keeps each item whose measurement justifies it. The items marked
built change the job database's layout, so every job runs all its steps once more: they are
measured on fresh runs of Dressrosa 11 and 28, the visual and localized steps judged on their own
outputs ([runbook](/documentation/runbooks/measuring_full_resolution_screening.md)).

## M6.5 — GPU-resident frames and a shared visual worker

Keep decoded frames where the next step needs them: on the GPU for the detector, in one process
across the visual steps, or in RAM between them. None of these is built; each waits for the
measurement it names.

- [ ] GPU decoding with frames kept on the GPU: NVDEC (Video Codec SDK through a Rust crate)
      decodes into device memory; YUV→RGB, padding and normalisation run as CUDA kernels and feed
      the TensorRT detector without a host round trip. Needs law 3 amended (FFmpeg stays for
      muxing, encoding, probing and the fallback). Built only if the TensorRT measurement shows
      decode, conversion or host↔device copies dominating `text_detect`. On the RTX 3070, NVDEC
      measured ~770 fps against ~1,500 fps CPU yuv420p (detect-bench, 5febb2b).
- [ ] One ONNX Runtime GPU process across the visual steps: `text_read`, `text_mask`,
      `text_inpaint` and `text_verify` (and `text_detect`) run in one long-lived worker that keeps
      their sessions loaded and a bounded frame cache, instead of one process per step. Runtimes
      stay apart (ggml and mistral.rs keep their own processes). Needs law 7 amended. Measured
      ceiling first: the summed `load_s` and decode time of those steps on Dressrosa 11 and 28.
- [ ] Shared frame cache in RAM: `text_detect` writes the decoded YUV frames of each occurrence's
      span (widened by the margins later steps read) to a memory-mapped file under `/dev/shm`
      (RAM), within a fixed budget (e.g. 4 GiB, part of the 24 GB); `text_mask`, `text_inpaint`
      and `text_verify` read frames from it instead of starting FFmpeg seeks, and fall back to
      FFmpeg for frames outside it. The cache is per job, removed when the job ends or is
      reopened, and never part of a fingerprint (decoded frames are bit-exact). Needs no change
      to law 7. Measured: those steps' decode time on Dressrosa 11 and 28 before and after.

Details: [memory profiles](/documentation/optimizations/memory_profiles.md#6-frames-kept-where-the-next-step-needs-them).

**Acceptance:** each item is built only once its measurement above shows it pays, and is then
measured on Dressrosa 11 and 28 against the M6 results in a research snapshot: per-step and
whole-job wall time, peak RAM within 24 GB, every GPU worker within 6.5 GB of VRAM, and the same
subtitle files and approved replacements. A law the item needs amended changes with its decision
entry, in the same commit. The owner keeps each item whose measurement justifies it.

## M7 — Dialogue accuracy and audio ensembling

Miss fewer spoken lines and spell names right more often: recover speech the backbone engine
missed, give Whisper and later episodes the names, add context across adjudication batches, and
measure better separation, a third engine and speaker turns.

- [ ] Separation settings: more overlap between windows (a shorter step), measured on what the
      vocal stem feeds: voice detection, alignment, the sound events and the re-decode. The stems
      stay 16 kHz, the rate every model after separation takes.
- [ ] Separation ensemble of the two built separators (Mel-Band RoFormer and MDX-Net Voc_FT);
      HTDemucs v4 only with an exported ONNX model.
- [ ] Orphan recovery in `diff_sheet`: speech another engine heard in a chunk where the backbone
      heard no word becomes an utterance for adjudication, instead of being dropped.
- [ ] The glossary as Whisper's initial prompt; needs Whisper driven through `whisper_full` or a
      CrispASR release whose session API takes a prompt.
- [ ] Learned glossary terms: a `terms` table in `library.redb`; spellings the owner keeps in
      Check Lines or Fix It join the glossary of later jobs.
- [ ] Context across first-pass adjudication batches: each batch of 60 also gets the last lines of
      the batch before as `CONTEXT` lines.
- [ ] Loudness normalisation and a raised consonant band for the `UNSURE` re-decodes.
- [ ] A third speech engine (such as Qwen3-ASR or Canary) on disputed utterances only.
- [ ] Speaker turns from a diarization model (Sortformer in parakeet-rs), given to adjudication.

Details: [audio accuracy](/documentation/optimizations/audio_accuracy.md).

**Acceptance:** against the baseline on Dressrosa 11 and 28, and on Dressrosa 11–48 for missed
speech and names: every utterance Whisper heard in a chunk Parakeet left empty is on the sheet and
becomes a cue or carries the language model's `DROP`; name disagreements, lines still unsure after
the re-decode and Claude calls are counted before and after each item built; the novelty check
finds no word an engine did not hear. The owner accepts the changed lines.

## M8 — Visual tracking and video acceleration

Make replaced writing follow moving signs, keep the erased background steady, and re-encode only
the segments of the localized video that change.

- [ ] A 3×3 homography per frame of moving writing in its `frames` row, used by composition; it
      addresses the 海 wall on Dressrosa 11, whose joined pieces cannot be followed by a shift and a
      scale ([polish record](/documentation/research/localized_video_polish_dressrosa_28.md#open-issues)).
- [ ] Keyframe inpaint and warped plates: LaMa fills one plate per shot, warped along the
      homography to the other frames, with a new plate where the warp stops matching.
- [ ] Re-encoding only what changed: segments with replaced writing, widened to IDR keyframes, are
      re-encoded as H.264 matching the source's profile, level and parameters (x264 by default,
      NVENC as a setting), with stream headers in-band, and joined to stream-copied H.264 by
      FFmpeg's concat demuxer; checks of frame count, duration, audio sync, decoding around each
      join and the copied bytes fall back to the HEVC encode of the whole video, which also serves
      every other constant-frame-rate source
      ([decision](/documentation/decisions/localized_video.md#2026-10-01--the-localized-video-re-encodes-only-the-segments-with-replaced-writing-as-h264-matching-the-source-and-copies-the-rest)).
      Built, awaiting the host measurement in the
      [runbook](/documentation/runbooks/measuring_full_resolution_screening.md).
- [ ] Smoothing the per-frame placement of moving writing over time (a Kalman filter or similar),
      and joining short fragments at fades to their occurrence; shot cuts already bound every
      occurrence.

Details: [visual and video](/documentation/optimizations/visual_and_video.md).

**Acceptance:** the 海 wall on Dressrosa 11 is replaced and approved by `text_verify`; against the
baseline on Dressrosa 11 and 28, no approved replacement is lost, and LaMa calls and the localized
video's time are recorded; a partially re-encoded localized video plays in VLC and mpv across every
join with its audio in sync and the source's frame count; the owner accepts moving replacements in
Check Text and in playback.

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

**Status:** live

# System overview

The shape of TBD-subtitles: an app binary with a job runner, separate ggml and local-language-model
worker binaries, FFmpeg for media, and a work directory per job. The 29-step pipeline runs from
the command line (`tbd-subtitles process`) or the window. A video occupies one queue entry for
dialogue, sound cues and on-screen Japanese translation. Enabled visual translation produces one
ASS file beside the source video; with the localized video on (the default for new jobs), the job
also writes a copy of the video with the Japanese writing replaced in English,
`<video>.localized.mkv`, and its own subtitle file, `<video>.localized.ass`. The source video is
only read.

The window includes visual settings, progress and timings, Overview counts and warnings, and
Check Text beside Check Lines. Check Text edits wording, timing and presentation, regenerates
the combined ASS, and compares source frames with FFmpeg rendering of that file, or with the
localized video and its own subtitle file (M5, under validation). These M4
components are integrated; annotated coverage, the episode benchmark, full GUI/VLC validation,
packaged host checks and owner acceptance remain in progress
([GUI](/documentation/features/gui.md),
[on-screen text](/documentation/features/japanese_onscreen_text.md)).

## Processes

```text
 user / Dolphin / watcher
            |
            v
 tbd-subtitles: GUI or CLI -> one job runner -> 29 resumable steps
            |                                      |
            | spawns assigned workers               | alongside initial decode
            v                                      v
      tbd-subtitles worker <step>                 tbd-subtitles worker shot_scan
      probe_decode · separation · asr_parakeet    (FFmpeg scdet, CPU)
      sound_events · alignment · redecode_parakeet
      review (CPU) · adjudicate · readjudicate · sound_cues (claude)
      text_detect · text_read · text_inpaint · text_verify (ONNX) · text_track (CPU)
      text_mask · text_compose (CPU) · localized_video (FFmpeg, NVENC)
      tbd-subtitles-ggml worker <step>
      asr_whisper · redecode_whisper
      tbd-subtitles-llm worker text_translate (mistral.rs; optional claude)
                            | reads and writes
                            v
                 work/<job id>/  job.redb, audio, visual artifacts, report.md
                            | text_review / text_typeset / QC / output in runner
                            v
                 <video folder>/<video base name>.ass
                 <video folder>/<video base name>.localized.mkv and .localized.ass
```

- **gui** — the eframe desktop app ([GUI feature](/documentation/features/gui.md)). It runs one
  full job at a time and, beside it, up to four correction runs of different videos at once, each
  on a runner thread of its own. Dialogue corrections run `review` on the CPU; visual corrections
  rerun the affected visual steps. Selected-occurrence retries can request OCR or translation
  workers. Background loading, model calls and preview rendering keep the window responsive.
- **process** — headless run for one or more files; used by the Dolphin entry and watch folders
  ([automation](/documentation/features/automation.md)).
- **worker `<step>`** — one step in its own process: it loads its model once, processes the whole
  job, takes the stored documents it reads as `Input` frames on stdin, sends its outputs as
  `Output` frames and its progress, model calls, own measure and end or failure to the runner as
  binary frames on a private copy of its stdout pipe
  ([worker channel](/crates/worker_channel/)), and exits; descriptor 1 points at stderr, so
  whatever a native library prints lands in the step log. This frees VRAM and keeps native
  libraries apart. ONNX models run in `tbd-subtitles`, Whisper in
  `tbd-subtitles-ggml`, and mistral.rs in `tbd-subtitles-llm`; the three binaries sit together.
  A worker dies with the process that started it (`PR_SET_PDEATHSIG`).
- **FFmpeg/ffprobe** — the media programs. Audio is decoded to a pipe (`-f f32le pipe:1`) and read
  in fixed-size chunks; video frames and presentation timestamps stream through bounded buffers.
  The scan retains representative crops and tracking data rather than every source image. Stderr
  is drained on its own thread. The optional `claude` CLI receives text or uncertain crops with
  tools disabled and shares one admission limit across worker processes and Fix It. There is
  no paid API backend or billing fallback.
- **Steps** — each stage is one or more steps with their own output, fingerprint and timing row.
  Detect → Read → Track → Translate → Review → Mask → Inpaint → Compose → Typeset follows
  dialogue cue construction and precedes final QC/output; the localized video is written last.
  A disabled visual branch, or a job with the localized video off, writes empty artifacts for
  those steps without loading models
  ([pipeline](/documentation/architecture/pipeline.md#steps-and-processes)).

## Crates

```text
apps/
├── tbd_subtitles/        the main binary: clap subcommands, eframe GUI, the job runner's front end,
│                         the ONNX Runtime, FFmpeg and claude workers
├── tbd_subtitles_ggml/   the ggml worker binary: the Whisper steps through CrispASR
└── tbd_subtitles_llm/    the mistral.rs worker binary: local visual translation
crates/
├── app_icon/             the application icon, painted in code as RGBA pixels
├── child_process/        external programs with deadlines, process-group kills, drained pipes, a
│                         stdin the caller streams into, and death with their parent
├── inference/            backends behind traits: onnx (ort), ggml, candle, llm (claude CLI, mistral.rs),
│                         model store, CUDA runtime locator
├── job_model/            serde and rkyv types for the job record, stage and step names, stage
│                         outputs, the QC report — the contracts
├── media_io/             ffprobe JSON, FFmpeg PCM and timestamped RGB streaming, region crops and
│                         native frames, shot-change scan, the localized-video encode
├── pipeline/             step graph, resume, work directory, worker processes, measurements, tasks,
│                         runner, progress events, report
├── stages/               one module folder per stage (probe_decode, separation, vad, asr, diff_sheet,
│                         sound_events, adjudication, alignment, cues, onscreen_text with its
│                         replace/ steps, qc, output, localize)
└── subtitle_formats/     cue model (frames), SRT/VTT/ASS writers and subtitle import
tools/
├── appimage_builder/     the three binaries and runtime libraries in one AppImage
├── repo_gates/           `cargo gates`: the repository laws a program can check
├── stack_spike*/         the stack spike's measuring harness and its ggml and llm workers
├── verification_core/    the fail-closed verdicts, patterns and reports the gates are written with
└── visual_validation/    annotated visual pilots and episode timing/memory measurements
```

Layering, lowest first: `job_model`, `child_process` and `app_icon`; `media_io`, `subtitle_formats` and
`inference`; `stages`; `pipeline`; the three app binaries. A crate
depends only on crates of a lower layer, never a sibling (`cargo gates crate-layering`); a tool
depends only on the crates listed for it in `tools/repo_gates/src/layout.rs`. Inside the app,
feature folders keep rendering out of their models and services (the tests in
`apps/tbd_subtitles/src/tests/architecture_rules.rs`). All boundaries are Rust to Rust, so the
stage contracts are the serde and rkyv types in `job_model`; the external contracts, the
language model's JSON answers, use structured schemas in the stage that makes the call.

## Job work directory

```text
work/<job id>/            <video file stem as a slug>-<8 hex of its path>
├── job.redb              the job database, open read-write by the one process running the job:
│                         the job record (meta/job_record), every step's record
│                         (step_records/<step>) and documents (outputs/<step>, plus
│                         outputs/cues/dropped_sounds and outputs/text_typeset/ass), the owner's
│                         line and text corrections and Fix It's record (corrections/lines, text,
│                         fix), and the per-frame tables
├── job.lock              the pid of that process, written once it has `job.redb` open
├── audio/                mix_16k.f32, vocals_16k.f32, background_16k.f32 (streamed, chunked)
├── sheet.txt             the diff sheet for reading
├── visual/               crops/ and keyframes/ of the detected writing; masks/, plates/ and
│                         patches/ of the writing replaced in the video; the reading cache
│                         readings/ and the translation cache translations/ (with the cached
│                         claude-*.json answers)
├── fix/calls/            Fix It's answered model calls, kept until a run finishes
├── report.md             QC results, flagged lines with timestamps, step timings and memory
├── logs/                 each worker's stderr
├── backup/               subtitle files the output step replaced, the localized one included
└── claude-cwd/           the empty folder `claude -p` runs in
```

A file outside the database is written and synced before the row that names it commits. When a
job's database opens, the files under `audio/` and `visual/{crops,keyframes,masks,plates,patches}`
that no row names are removed; the caches, `fix/calls/`, `logs/`, `sheet.txt` and `report.md`
stay. A job from before the database is not imported: it runs again.

A step is skipped when its step record holds its current fingerprint, its documents are stored
and the files its rows name exist. The fingerprint covers the settings the step reads, the stored
table layouts and what its inputs were, so a changed setting reruns only its dependent steps.
Visual fingerprints also cover pinned model identities and
installed-file metadata, reference ASS contents, and the relevant corrections. English wording
or placement edits invalidate visual review/typesetting and final output; selected retries also
invalidate reading/translation. Valid audio artifacts remain reusable. A step records the
fingerprint captured before execution, so an edit during a run still makes its result stale.
A missing file a step's rows name reruns that step and its dependants; `--rerun <step>` clears
that step and every step that reads it in one transaction.

Records without visual settings keep that branch disabled until an explicit rerun enables it,
and records without the localized-video setting keep the localized video off. New jobs enable
both by default and select ASS. Turning the localized video on or off reruns only the
replacement steps, typesetting, the quality check, the output and the localized video, never
translation. Replacing a job's SRT with ASS uses the subtitle
backup mechanism. Uncertain text completes with review flags; infrastructure failures remain
explicit and resumable.

## Models

Downloaded on first use into a models folder (default `~/.local/share/tbd-subtitles/models/`,
configurable), each with a pinned source URL and checksum in a manifest compiled into the binary.
No model conversion ever happens locally.

On-screen detection and recognition use PP-OCRv5 through `oar-ocr`, with manga-ocr for difficult
crops. Qwen3.5-4B supplies local translations with dialogue context and the glossary. Reference
ASS wording is reused only after a scene and content match; its timing and geometry are not
copied. Claude image input can resolve uncertain crops when enabled. Typesetting uses ordinary
ASS text or vector glyphs for perspective. Uncertain surfaces receive nearby translations and
flags; ASS cannot reconstruct hidden artwork. The localized video can: LaMa (`lama-inpaint`,
`Carve/LaMa-ONNX` `lama_fp32.onnx`, 208 MB, on ONNX Runtime) fills the erased strokes, and
Noto Sans (`latin-fonts`, the variable font with its OFL licence) letters the English, drawn with
tiny-skia. Both are required only while the localized video is on.

## Configuration

The settings file `~/.config/tbd-subtitles/settings.toml` (TOML; `XDG_CONFIG_HOME` moves it)
holds the owner's choices: the models folder, the work folder, the glossary (the built-in One
Piece glossary by default), the separator and Whisper model, the language-model backend, the
model a run asks and the one Fix It asks, the process count, the shared `claude` call cap,
whether Fix It starts on each video when its job finishes, the cut score and the output format.
The On-screen Text section controls visual processing, the localized video (Replace text in the
video; a settings file without the key means on), Claude fallback, the reference ASS folder and
model availability/downloads; local translation runs first. Enabled visual processing takes
precedence over the selected format and writes ASS. A missing file means the defaults; an
unknown key or a bad value stops the run with its name. The window edits the file and the command
line reads it; `tbd-subtitles process --help` lists the options that win over it for one run,
plus the audio track and the steps to run again. The job record keeps the job's settings in
`job.redb` (`meta/job_record`), so a resumed job knows what its outputs were made with.

## Hardware and host rules

- GPU stages need the host's NVIDIA driver; inside the `claude-desktop` distrobox they are run
  through `distrobox-host-exec` during development.
- ONNX Runtime (Microsoft's CUDA 13 build), the CUDA 13 runtime and cuDNN live in the runtime
  folder `~/.local/share/tbd-subtitles/runtime/` (or `<binary folder>/cuda/`), not installed
  system-wide. GPU workers receive the CUDA library path; only ONNX workers receive
  `ORT_DYLIB_PATH`. The mistral.rs worker does not load ONNX Runtime.
- ONNX Runtime, ggml and mistral.rs stay in separate worker binaries.
- Resource limits: 8 GB RAM and 5.5 GB VRAM per GPU worker with the desktop running. Visual
  processing adds measured time to the audio pipeline.
- One GPU worker at a time on the machine: every GPU worker holds `gpu.lock` in the app data
  folder while it runs, whichever process of the app started it. The inpainting worker and the
  localized-video worker, whose FFmpeg encodes with NVENC, hold it too.

The completed M1 audio benchmark is 128.9 minutes of video in 19.2 minutes against a
30-minute target for two hours ([measurement](/documentation/research/long_video_120min.md)).
It contains no visual workload. M4 acceptance uses one 20–30-minute episode, as requested by the
owner in place of the two-hour visual benchmark, and reports the added visual processing time
and peak memory. That validation and owner acceptance are outstanding. The localized video adds
about 4.9 minutes to a 30.9-minute episode, 4.5 of them encoding
([measurement](/documentation/research/localized_video_dressrosa_11.md)).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the crates and model files.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  replacement steps and the localized video.
- [Development environment](/documentation/runbooks/development_environment.md) — host, container
  and toolchain facts.

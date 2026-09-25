**Status:** live

# System overview

The planned shape of TBD-subtitles: one Rust binary, a job runner, worker processes for GPU
stages, FFmpeg for media, and a work directory per job. Nothing here exists yet; milestone M0
creates the skeleton.

## Processes

```text
            ┌──────────────── tbd-subtitles (one binary) ────────────────┐
 user ────▶ │ gui      eframe window: queue, progress, reports, review   │
 Dolphin ─▶ │ process  CLI: run one video end to end                     │
 watcher ─▶ │          job runner: stages in order, resume, report       │
            └───────────────┬───────────────────────┬─────────────────────┘
                            │ spawns, one at a time │ spawns (CPU, parallel)
                            ▼                       ▼
                 worker <gpu stage>          ffmpeg / ffprobe
                 separate · asr · align      decode · probe · shot changes
                 sound_events · llm · ocr
                            │ reads and writes
                            ▼
                 work/<job id>/  stage outputs (JSON + audio), report
                            │ final stage
                            ▼
                 <video folder>/<video base name>.srt|.ass
```

- **gui** — the eframe desktop app ([GUI feature](/documentation/features/gui.md)).
- **process** — headless run for one or more files; used by the Dolphin entry and watch folders
  ([automation](/documentation/features/automation.md)).
- **worker `<stage>`** — a GPU stage in its own process: loads one model, processes all
  chunks of the job, writes its output, exits. Frees VRAM and keeps native libraries apart.
- **FFmpeg/ffprobe** — the only external programs. Audio is decoded to a pipe
  (`-f f32le pipe:1`) and read in fixed-size chunks; stderr is drained on its own thread.

## Planned crates

```text
apps/
└── tbd_subtitles/        the binary: clap subcommands, eframe GUI, composition root
crates/
├── job_model/            serde types for jobs, stage outputs, reports — the contracts between stages
├── media_io/             ffprobe JSON, FFmpeg PCM streaming, shot-change scan, chunking
├── pipeline/             stage graph, resume logic, worker spawning, progress events
├── stages/               one module folder per stage (separation, vad, asr, adjudication,
│                         alignment, cues, sound_events, qc, output)
├── inference/            backends behind traits: onnx (ort), ggml, candle, llm (claude CLI, mistral.rs)
└── subtitle_formats/     cue model, SRT/ASS/VTT writers, import of existing subtitles
tools/
└── repo_gates/           checks the repository laws (language ban, file length, READMEs, links)
```

Layering: `tbd_subtitles` → `pipeline` → `stages` → `inference`, `media_io`,
`subtitle_formats` → `job_model`. A lower crate never depends on a higher one; architecture tests
hold this. All boundaries are Rust to Rust, so the stage contracts are the serde types in
`job_model`; the one external contract, the language model's JSON answer, is a JSON Schema kept
beside its backend.

## Job work directory

```text
work/<job id>/
├── job.json              input path, probe result, settings, stage status and timings
├── audio/                mix_16k.f32, vocals_16k.f32, background_16k.f32 (streamed, chunked)
├── shots.json            shot-change times
├── vad.json              speech regions and chunk plan
├── asr/<engine>.json     per-engine words with times and confidences
├── sheet.txt             the diff sheet for adjudication
├── adjudicated.json      final text per utterance, flags
├── aligned.json          final words with times and their timing source
├── sound_events.json     candidate and chosen sound cues
├── cues.json             finished cues
└── report.md             QC results, flagged lines with timestamps, stage timings
```

A stage is skipped when its output exists and its recorded inputs and settings are unchanged.
Deleting a stage's output reruns it and everything after it.

## Models

Downloaded on first use into a models folder (default `~/.local/share/tbd-subtitles/models/`,
configurable), each with a pinned source URL and checksum in a manifest compiled into the binary.
No model conversion ever happens locally.

## Configuration

A TOML settings file (default `~/.config/tbd-subtitles/settings.toml`): models folder, work
folder, engines per stage, language-model backend, output format, watch folders. The GUI edits it;
the CLI reads it and accepts overrides.

## Hardware and host rules

- GPU stages need the host's NVIDIA driver; inside the `claude-desktop` distrobox they are run
  through `distrobox-host-exec` during development.
- `ort`'s prebuilt GPU build needs CUDA 13 runtime libraries (cudart, cuBLAS, cuDNN); on Bazzite
  they ship beside the binary rather than being installed system-wide.
- Budget per GPU stage: 5.5 GB of VRAM with the desktop running.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the crates and model files.
- [Development environment](/documentation/runbooks/development_environment.md) — host, container
  and toolchain facts.

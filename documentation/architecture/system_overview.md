**Status:** live

# System overview

The shape of TBD-subtitles: an app binary with a job runner, a second binary for the ggml worker,
worker processes for the GPU and language-model steps, FFmpeg for media, and a work directory per
job. The pipeline runs end to end from the command line (`tbd-subtitles process`) or from the
window, which queues videos, runs their jobs one at a time with progress and the time left, shows
each job's report and lets the owner review and correct its flagged lines
([GUI](/documentation/features/gui.md)).

## Processes

```text
            ┌──────────────────── tbd-subtitles ──────────────────────────┐
 user ────▶ │ gui      eframe window: queue, progress, reports, review    │
 Dolphin ─▶ │ process  CLI: run each video end to end                     │
 watcher ─▶ │          job runner: steps in order, resume, measure, report│
            └───────────────┬─────────────────────────────┬───────────────┘
                            │ spawns, one at a time       │ spawns alongside
                            ▼                             ▼
      tbd-subtitles worker <step>                 tbd-subtitles worker shot_scan
      probe_decode · separation · asr_parakeet    (FFmpeg scdet, CPU)
      sound_events · alignment · redecode_parakeet
      review (CPU) · adjudicate · readjudicate · sound_cues (claude)
      tbd-subtitles-ggml worker <step>
      asr_whisper · redecode_whisper
                            │ reads and writes
                            ▼
                 work/<job id>/  step outputs (JSON + audio), job.json, report.md
                            │ output step
                            ▼
                 <video folder>/<video base name>.srt
```

- **gui** — the eframe desktop app ([GUI feature](/documentation/features/gui.md)).
- **process** — headless run for one or more files; used by the Dolphin entry and watch folders
  ([automation](/documentation/features/automation.md)).
- **worker `<step>`** — one step in its own process: it loads its model once, processes the whole
  job, writes its output and its own measure (`steps/<step>.worker.json`), and exits. This frees
  VRAM and keeps native libraries apart. ONNX Runtime, ggml and candle never share a binary, so
  the Whisper steps run in `tbd-subtitles-ggml`, which sits beside `tbd-subtitles`. A worker dies
  with the process that started it (`PR_SET_PDEATHSIG`).
- **FFmpeg/ffprobe** — the media programs. Audio is decoded to a pipe (`-f f32le pipe:1`) and read
  in fixed-size chunks; stderr is drained on its own thread. The `claude` CLI is the other
  external program, for the language-model steps.
- **Steps** — each stage is one or more steps with their own output, fingerprint and timing row
  ([pipeline](/documentation/architecture/pipeline.md#steps-and-processes)).

## Crates

```text
apps/
├── tbd_subtitles/        the main binary: clap subcommands, eframe GUI, the job runner's front end,
│                         the ONNX Runtime, FFmpeg and claude workers
└── tbd_subtitles_ggml/   the ggml worker binary: the Whisper steps through CrispASR
crates/
├── child_process/        external programs with deadlines, process-group kills, drained pipes, and
│                         death with their parent
├── inference/            backends behind traits: onnx (ort), ggml, candle, llm (claude CLI, mistral.rs),
│                         model store, CUDA runtime locator
├── job_model/            serde types for the job record, stage and step names, stage outputs, the QC
│                         report — the contracts
├── media_io/             ffprobe JSON, FFmpeg PCM streaming, shot-change scan
├── pipeline/             step graph, resume, work directory, worker processes, measurements, tasks,
│                         runner, progress events, report
├── stages/               one module folder per stage (probe_decode, separation, vad, asr, diff_sheet,
│                         sound_events, adjudication, alignment, cues, qc, output)
└── subtitle_formats/     cue model (frames), SRT writer; VTT/ASS writers and import to come
tools/
├── repo_gates/           `cargo gates`: the repository laws a program can check
├── stack_spike*/         the stack spike's measuring harness and its ggml and llm workers
└── verification_core/    the fail-closed verdicts, patterns and reports the gates are written with
```

Layering, lowest first: `job_model` and `child_process`; `media_io`, `subtitle_formats` and
`inference`; `stages`; `pipeline`; the apps `tbd_subtitles` and `tbd_subtitles_ggml`. A crate
depends only on crates of a lower layer, never a sibling (`cargo gates crate-layering`); a tool
depends only on the crates listed for it in `tools/repo_gates/src/layout.rs`. Inside the app,
feature folders keep rendering out of their models and services (the tests in
`apps/tbd_subtitles/src/tests/architecture_rules.rs`). All boundaries are Rust to Rust, so the
stage contracts are the serde types in `job_model`; the external contracts, the language model's
JSON answers, are JSON Schemas kept beside their prompts in `crates/stages/src/adjudication/`.

## Job work directory

```text
work/<job id>/            <video file stem as a slug>-<8 hex of its path>
├── job.json              the video, its size and time, the settings, and each finished step's
│                         fingerprint, finish time and measure
├── job.lock              the pid of the run holding the job
├── probe.json            the probe result and the decoded audio track
├── audio/                mix_16k.f32, vocals_16k.f32, background_16k.f32 (streamed, chunked)
├── shots.json            every shot change with its scdet score
├── vad.json              speech regions and chunk plan
├── asr/                  parakeet.json, whisper.json: words with times and confidences
├── sheet.json, sheet.txt the diff sheet for adjudication
├── sound_events.json     the detector's events on both stems
├── adjudication/         first.json (first pass), redecode_parakeet.json, redecode_whisper.json
├── adjudicated.json      final text per utterance, flags and the checks' findings
├── sound_cues.json       candidates and the chosen, worded sound cues
├── aligned.json          final words with times and their timing source
├── review.json           the owner's corrections, written by the window
├── reviewed.json         the aligned words with the corrected lines timed again
├── cues.json             finished cues, in frames (and cues_dropped_sounds.json)
├── qc.json               the quality check
├── output.json           where the subtitle file went and what it replaced
├── report.md             QC results, flagged lines with timestamps, step timings and memory
├── steps/, logs/         each worker's own measure and its stderr
├── backup/               subtitle files the output step replaced
└── claude-cwd/           the empty folder `claude -p` runs in
```

A step is skipped when `job.json` holds its current fingerprint and its outputs exist. The
fingerprint covers the settings the step reads and what its inputs were, so a changed setting
reruns only the steps that read it and the steps after them. Deleting a step's output, or
`--rerun <step>`, reruns it and everything after it.

## Models

Downloaded on first use into a models folder (default `~/.local/share/tbd-subtitles/models/`,
configurable), each with a pinned source URL and checksum in a manifest compiled into the binary.
No model conversion ever happens locally.

## Configuration

The settings file `~/.config/tbd-subtitles/settings.toml` (TOML; `XDG_CONFIG_HOME` moves it)
holds the owner's choices: the models folder, the work folder, the glossary (the built-in One
Piece glossary by default), the separator and Whisper model, the language-model backend, model
and process count, the cut score and the output format. A missing file means the defaults; an
unknown key or a bad value stops the run with its name. The window edits the file and the command
line reads it; `tbd-subtitles process --help` lists the options that win over it for one run,
plus the audio track and the steps to run again. The job record keeps the job's settings in
`job.json`, so a resumed job knows what its outputs were made with.

## Hardware and host rules

- GPU stages need the host's NVIDIA driver; inside the `claude-desktop` distrobox they are run
  through `distrobox-host-exec` during development.
- ONNX Runtime (Microsoft's CUDA 13 build), the CUDA 13 runtime and cuDNN live in the runtime
  folder `~/.local/share/tbd-subtitles/runtime/` (or `<binary folder>/cuda/`), not installed
  system-wide; each GPU worker starts with `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` pointing there.
- ONNX Runtime, ggml and candle never share a binary: each GPU runtime has a worker binary of its
  own.
- Budget per GPU stage: 5.5 GB of VRAM with the desktop running.
- One GPU worker at a time on the machine: every GPU worker holds `gpu.lock` in the app data
  folder while it runs, whichever process of the app started it.

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md) — what each stage does.
- [Rust ML stack](/documentation/research/rust_ml_stack.md) — the crates and model files.
- [Development environment](/documentation/runbooks/development_environment.md) — host, container
  and toolchain facts.

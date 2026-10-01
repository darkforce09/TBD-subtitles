**Status:** live

# Decisions: foundations

The decisions that set the project up: what is built, in what language, with which external
programs and runtimes, how the work is documented, and where subtitles go. The
[decision log](/documentation/decisions/) says how entries are written.

### 2026-09-25 — Build a reusable local app, not a one-off script

**Context:** The owner needs English subtitles for the Muhn Pace Dressrosa dub (41 episodes) and
expects to download more videos without subtitles. A one-off transcription run was planned first.

**Decision:** Build TBD-subtitles, a desktop application that processes any video on demand, with
Dressrosa 08 as the pilot and 09–48 as the first batch.

**Consequences:** The pipeline is written once as product code with a GUI and CLI, not as throwaway
commands. The first subtitles arrive later than a one-off run would have delivered them.

**Supersedes:** none.

### 2026-09-25 — Rust only; FFmpeg as the one external program

**Context:** The owner wants a pure-Rust application: no Python or other languages anywhere, and
FFmpeg for decoding instead of a custom decoder. Most speech tooling is Python-first.

**Decision:** All project code is Rust. No Python, shell, Makefiles or Node in the repository, for
tooling included. FFmpeg and ffprobe are called as child processes. Models are downloaded already
exported (ONNX, GGUF, safetensors) and never converted by us.

**Consequences:** Python-only tools (qwen-asr, WhisperX, audio-separator, NeMo, PANNs) are out.
Their Rust counterparts are listed in the [Rust ML stack](/documentation/research/rust_ml_stack.md).
A Rust gate program enforces the language ban once code exists.

**Supersedes:** none.

### 2026-09-25 — Native inference runtimes through Rust crates are allowed (to confirm)

**Context:** Pure-Rust engines (candle, burn, mistral.rs, earshot) cover the language models and
voice detection well, but the fastest and most accurate options for speech recognition, vocal
separation and forced alignment on CUDA are ONNX Runtime (through `ort`) and ggml (through
whisper-rs or transcribe-cpp), which are C/C++ libraries behind Rust crates.

**Decision:** Use pure-Rust engines first; use `ort` or ggml crates only where no pure-Rust option
is competitive. This reading of "pure Rust" awaits the owner's confirmation. If the owner rules
native runtimes out, the strict path is: Kyutai STT through candle for speech, demucs-rs (burn)
for separation, the pure-Rust Qwen3 aligner ports for alignment.

**Consequences:** The project code stays Rust; binaries link native libraries. `ort` pins one
ONNX Runtime version per binary and its prebuilt GPU build needs CUDA 13 libraries shipped beside
the app; ggml crates each bundle ggml, so no two share a binary. Both are handled by running each
GPU stage as its own process.

**Supersedes:** none.

### 2026-09-25 — Local speech recognition; cloud APIs declined

**Context:** On the Artificial Analysis word-error index, cloud models lead (Microsoft
MAI-Transcribe-2 2.0 %, ElevenLabs Scribe v2 2.2 % at $0.22 per hour) and open models trail
(Whisper large-v3 4.1 %, Canary-Qwen-2.5B 4.3 %, Parakeet TDT 0.6B v2 6.4 %). Scribe v2 would have
cost about $6 for the 19.8 hours of Dressrosa. The owner chose free and local.

**Decision:** All speech recognition runs locally on the owner's GPU. Accuracy is recovered with
an ensemble of engines, language-model adjudication and targeted re-decoding.

**Consequences:** No accounts, no costs, no uploads. A cloud backend can be added later behind the
same backend interface if local accuracy falls short.

**Supersedes:** none.

### 2026-09-25 — SDH subtitles: dialogue plus sound cues, no lyrics

**Context:** The owner asked for dialogue plus sound cues and no song lyrics. The Dressrosa
opening is a Japanese song, and the owner's playlist skips it.

**Decision:** Output English SDH: every spoken line, plus bracketed lowercase sound cues
(`[explosion]`, `[laughs]`) where they matter. Songs get one music cue, not lyrics. Pure filler
(uh, um) is dropped; hesitations that carry meaning are kept.

**Consequences:** A sound-event detector is part of the pipeline, and a language model curates
its output so cues stay sparse.

**Supersedes:** none.

### 2026-09-25 — Simple documentation system, no ticket machinery

**Context:** The owner's TBD-Factory template (ticket engine, waves, xtask gates) is too complex
for this app, and its latest improvements live only in TBD-Reforger. The owner still wants
systematic work.

**Decision:** Use a trimmed copy of TBD-Reforger's `documentation_v2` conventions in
`documentation/`, with a milestone roadmap of checklists instead of tickets. No TBD-Factory, no
ticketboard, no `.ai/` ticket registry, no xtask.

**Consequences:** Progress is tracked by ticking roadmap checklists in the same commit as the
work. Repository rules that a program can check run in a small Rust gate tool (`tools/repo_gates`).

**Supersedes:** none.

### 2026-09-25 — Each GPU stage runs in its own worker process

**Context:** The GPU has about 5.5 GB free. Separation, speech recognition, alignment and a local
language model do not fit together, and native runtimes clash inside one binary.

**Decision:** The app binary exposes each GPU stage as a worker subcommand. The job runner starts
one worker per stage, which loads its model, processes the job's chunks, writes its output, and
exits.

**Consequences:** VRAM is freed between stages; a crash loses one stage, not the job; libraries
with conflicting native dependencies can live in separate binaries if needed.

**Supersedes:** none.

### 2026-09-25 — Subtitles live beside the video with the same base name

**Context:** VLC loads a subtitle file automatically when it sits next to the video with the same
base name, and the owner wants no manual steps.

**Decision:** Output `<video base name>.srt` (or `.ass` when positioned sign subtitles exist) in
the video's folder, UTF-8, one subtitle file per video. Source videos are never modified.

**Consequences:** Existing subtitle files are backed up before being replaced. Work files go to
the job's work directory, never into the media folder.

**Supersedes:** none.

### 2026-09-25 — Native inference runtimes allowed where no pure-Rust engine competes

**Context:** The earlier entry allowed ONNX Runtime (through `ort`) and ggml (through whisper-rs,
transcribe-cpp or crispasr) pending the owner's confirmation. The owner confirmed it.

**Decision:** Inference uses pure-Rust engines (candle, burn, mistral.rs, earshot) first. Rust
crates that bind ONNX Runtime or ggml are used where no pure-Rust option is competitive in speed or
accuracy. All project code stays Rust.

**Consequences:** Parakeet ONNX, MDX-Net and Mel-Band RoFormer separation on CUDA, CED sound events
and the Qwen3 aligner through crispasr are all open to the M0.5 spike. CUDA 13 runtime libraries
ship beside the app, and each GPU stage keeps its own worker process so native libraries never
share a binary.

**Supersedes:** 2026-09-25 — Native inference runtimes through Rust crates are allowed (to confirm).

### 2026-09-25 — Repository tooling may run git and cargo

**Context:** The repository gates copied from TBD-Reforger list tracked files with `git ls-files`,
ask `git check-ignore` about ignored paths, and build temporary git checkouts in their tests. The
language law names FFmpeg and ffprobe as the only external programs.

**Decision:** Programs under `tools/` may run `git` and `cargo` as child processes. The app
(`apps/` and `crates/`) still runs only FFmpeg, ffprobe and, as an optional language-model backend,
the `claude` CLI.

**Consequences:** The gates judge exactly the tracked files, as in TBD-Reforger, and are copied
nearly as they are. A gate run needs `git` on the `PATH`; the app does not.

**Supersedes:** none.

### 2026-09-25 — Dressrosa 11 is the pilot; 12–48 are the first batch

**Context:** The owner has watched Dressrosa 08–10 and moved them to `one_pace/done/`, so a
subtitle file for them would not be watched.

**Decision:** The stack spike and the pilot run on `[Muhn Pace] Dressrosa 11.mp4`. After the owner
accepts the pilot in VLC, episodes 12–48 are processed as the first batch.

**Consequences:** The spike's measurements, the pilot subtitle file and the batch all use episodes
still in the main media folder. Episodes 08–10 get no subtitles unless the owner asks.

**Supersedes:** 2026-09-25 — Build a reusable local app, not a one-off script (its choice of pilot
and batch only).

### 2026-09-28 — Repository tooling may also run FFmpeg and the app's own binaries

**Context:** `tools/appimage_builder` packages the app as an AppImage and must prove the bundled
FFmpeg build it downloads actually works (version, `scdet` and `apad` filters, the `pulse`
device), and that the `tbd-subtitles` and `tbd-subtitles-ggml` binaries it just built and staged
run before they are packed into the squashfs image. Both checks mean running those programs as
child processes from a `tools/` program, which the git-and-cargo-only entry does not allow.

**Decision:** Programs under `tools/` may also run FFmpeg (to verify a downloaded or bundled
build) and the app's own built binaries (to smoke-test a staged `AppDir` before packing).
Packaging stays pure Rust: no `appimagetool`, `patchelf` or `mksquashfs` — ELF rewriting and
squashfs assembly are done with Rust crates (`object`, `backhand`), never shelled-out tools.

**Consequences:** `tools/appimage_builder` runs `git`, `cargo`, `ffmpeg`/`ffprobe` and the staged
`usr/bin/tbd-subtitles`/`tbd-subtitles-ggml` as child processes; no other new external program is
introduced. The app itself (`apps/`, `crates/`) is unchanged: still only FFmpeg, ffprobe and,
optionally, `claude`.

**Supersedes:** 2026-09-25 — Repository tooling may run git and cargo (widens what `tools/`
programs may run as child processes).


### 2026-09-30 — The pipeline targets the owner's 32 GB machine: 24 GB of RAM

**Context:** The 8 GB RAM limit kept the prototype lean, but it rules out full-resolution text
screening, per-frame data held for a whole shot, longer separation windows and steps running side
by side. The app runs on one machine, the owner's: an i7-14700K with 32 GB of DDR5-6000 and an
RTX 3070. Designing for 8 GB or 16 GB machines serves no user.

**Decision:** Peak RAM stays within 24 GB, leaving 8 GB to the desktop; each GPU worker stays
within 5.5 GB of VRAM, the card's 8 GB less the display's share. Memory stays bounded: audio and
frames are streamed or held in bounded windows, never grown with the video without limit.

**Consequences:** Law 5 of CLAUDE.md and the memory row of the success criteria change. Work
limited by the GPU (the detector, LaMa, speech models, the local translation model) gains nothing
from the extra RAM; the VRAM cap still decides which models fit. Each use of the headroom is
measured on a real episode before it is kept.

**Supersedes:** the 8 GB RAM limit of law 5 and of the success criteria in
[vision and goals](/documentation/vision_and_goals.md).

### 2026-10-01 — Each GPU worker stays within 6.5 GB of VRAM, and a step waits, up to a deadline, for the memory it measured

**Context:** The detect-bench (`tools/visual_validation/src/detect_bench/`) showed
full-resolution text screening bound by the GPU near 35 frames a second with one detector
session, and the owner decided to run two detector sessions in the one `text_detect` worker. Two
sessions' memory pools and their CUDA context do not fit in 5.5 GB. The card is an RTX 3070 with
8 GiB, of which the desktop uses about 1.2–1.9 GB, so a worker has between about 6.3 and 7 GB
before anything else runs. Each GPU step so far started as soon as it held the GPU lock: when
another program held VRAM, the step's model failed to load or ran out of memory part-way, and the
job crashed. The options were to keep 5.5 GB and one session, to raise the cap and let a short
card crash the job, or to raise the cap and have each step wait for the memory it needs.
Translation held the GPU lock for its whole run although Claude answers most keyframes and the
local model loads only for what Claude leaves; meanwhile the main walk's audio steps, which delay
the whole job, could queue behind the visual lane's steps.

**Decision:** Each GPU worker stays within 6.5 GB (6,656 MiB, `graph::WORKER_VRAM_CAP_MIB`) of
VRAM. Each GPU step needs its measured peak on Dressrosa 11 and 28
([baseline](/documentation/research/m6_baseline.md)) plus 256 MiB (`graph::vram_need_mib`),
never the cap; `text_detect`'s need is `graph::TEXT_DETECT_VRAM_MIB`, which the host's pool-by-batch
sweep sets. After a step takes the GPU lock it reads the device's free memory through NVML every
second until it reaches the need, showing "waiting for GPU memory: N MiB free, M needed" once and
again on each change of 128 MiB or more; cancelling the job ends the wait; after 10 minutes the
step fails, naming the free memory and the need and saying "close other GPU programs and retry".
Without NVML there is no check. A main-walk step waiting for the lock goes before every
visual-lane step, in this process or another (`graph::gpu_priority`). `text_translate` takes the
lock and the memory wait only when its local model loads, in its worker, and holds both until the
model drops; its waiting lines reach the runner as `Message` frames of the worker channel.

**Consequences:** Law 5 of CLAUDE.md says 6.5 GB. The machine check in Settings warns below
6,656 MiB free, and the stack spike refuses a GPU item below it. A job on a card that another
program fills waits, saying so, instead of crashing, and fails after 10 minutes rather than waiting
silently; a step whose need fits beside the desktop never waits. A visual-lane step may wait longer
behind the audio steps. The needs are measured values: a model change that raises a step's peak
raises its need in the same commit. The wait, the priority and the lazy lock are held by the tests
of `crates/pipeline/src/workers/` (`tests/vram_guard.rs`, `tests/gpu_lock.rs`,
`tests/lazy_gpu.rs`) and the graph's `tests/gpu.rs`.

**Supersedes:** the 5.5 GB VRAM part of
[2026-09-30 — The pipeline targets the owner's 32 GB machine: 24 GB of RAM](/documentation/decisions/foundations.md#2026-09-30--the-pipeline-targets-the-owners-32-gb-machine-24-gb-of-ram);
its 24 GB of RAM stands.

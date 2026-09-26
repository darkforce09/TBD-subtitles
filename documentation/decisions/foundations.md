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


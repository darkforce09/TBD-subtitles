**Status:** live

# Decisions

The log of choices that shape TBD-subtitles and would otherwise be argued again. Each entry says
what was decided, why, what follows, and which earlier entry it replaces. A changed decision gets a
new entry that names the old one under Supersedes. Open questions that are not yet decided live in
the [roadmap](/documentation/roadmap.md#open-questions).

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

### 2026-09-26 — ONNX Runtime is loaded at run time from Microsoft's CUDA 13 build

**Context:** The `ort` crate's prebuilt static ONNX Runtime needs glibc 2.38 and GCC 13's
libstdc++ to link; the `claude-desktop` build container has glibc 2.36 and GCC 12, so nothing that
used `ort` linked there. Building on the host (cmake via Homebrew, nvcc forced past GCC 16) and a
new Ubuntu build container were the other options.

**Decision:** `ort` runs with `load-dynamic` and loads Microsoft's
`onnxruntime-linux-x64-gpu_cuda13-1.28.2` release at run time. It, the CUDA 13.4 runtime and cuDNN
9.26 live in `~/.local/share/tbd-subtitles/runtime/`, downloaded by the model store with pinned
SHA-256; a packaged `<binary folder>/cuda/` with the same layout is looked in first. Every GPU worker
starts with `LD_LIBRARY_PATH` set to the three `lib/` folders and `ORT_DYLIB_PATH` to
`libonnxruntime.so`.

**Consequences:** Everything keeps building in the container; the host is not touched. ONNX
Runtime is chosen by its release asset, not by the `ort` crate's version pin. `CudaRuntime::locate`
refuses a runtime folder missing any library. `ort`'s `preload-dylibs` is not used: rc.13 still
names CUDA 12 libraries there.

**Supersedes:** none.

### 2026-09-26 — The stack spike tool drives the product crates and its own workers

**Context:** The repository tools may run `git` and `cargo`. Measuring the stack needs FFmpeg,
ffprobe, the `claude` CLI and GPU worker processes.

**Decision:** `tools/stack_spike` and `tools/stack_spike_ggml` may use the product crates
`child_process`, `job_model`, `media_io`, `inference` and `stages`, which run FFmpeg, ffprobe and
`claude` as the app does; the tool itself starts only its own binaries as workers.

**Consequences:** The tool table in `tools/repo_gates/src/layout.rs` lists these crates for the two
tools (`cargo gates crate-layering`). The tools write only under
`~/.local/share/tbd-subtitles/`; the video is only read.

**Supersedes:** none.

### 2026-09-26 — Each native GPU runtime lives in a worker binary of its own

**Context:** Loading ONNX Runtime into a process that also links CrispASR (ggml) aborted with heap
corruption; so did several ONNX Runtime workers (RoFormer, Parakeet, Parakeet-CTC) in a binary
that also linked mistral.rs (candle, cudarc), while others in the same binary passed. The earlier
entry kept GPU stages in separate processes, not separate binaries.

**Decision:** ONNX Runtime, ggml and candle are never linked into the same binary. The stack spike
has three: `stack-spike` (ONNX Runtime), `stack-spike-ggml` (CrispASR) and `stack-spike-llm`
(mistral.rs); the app gets one worker binary per runtime in the same way.

**Consequences:** Each binary has its own Cargo feature and build environment. Items name the
binary that runs them (`Item::worker_binary`), and the parent measures them all the same way.
Shared code stays in the product crates, so the workers stay small.

**Supersedes:** none.

### 2026-09-26 — Mel-Band RoFormer separates by default; MDX-Net Voc_FT is the fast mode

**Context:** On Dressrosa 11 MDX-Net Voc_FT separated in 52.7 s (3.4 min per 120 min, 2.4 GB VRAM)
and Mel-Band RoFormer in 99.8 s (6.5 min, 4.3 GB), both inside the 12-minute budget. RoFormer
left about a quarter less music in its vocal stem (mean CED Music score 0.030 against 0.041). No
reference stems exist to judge further.

**Decision:** The separation stage uses Mel-Band RoFormer (host-STFT ONNX export, 11 s windows,
Hamming overlap-add with an 8 s step). MDX-Net Voc_FT, at batch 1, is the fast mode. Both run on our
own STFT and streaming overlap-add; stems are written as 16 kHz mono.

**Consequences:** The vocal stem that feeds voice activity, alignment and the vocal half of sound
events is RoFormer's. The owner's listening of the WAV excerpts in the spike work folder may
revisit the choice.

**Supersedes:** none.

### 2026-09-26 — Whisper large-v3 through CrispASR is the second speech engine

**Context:** The open question named Whisper large-v3, Canary or Granite, and Kyutai STT 1B. The
owner chose to measure Whisper large-v3 through CrispASR only. It ran at 25× realtime (4.8 min per
120 min, 4.4 GB VRAM); large-v3-turbo q8_0 ran at 99× (1.2 min, 1.4 GB). Whisper and Parakeet
disagreed on 9.9 % of words, and Whisper kept interjections Parakeet drops.

**Decision:** The second engine is Whisper large-v3 (`ggml-large-v3.bin`) through CrispASR, in the
ggml worker binary, over the same chunk plan as Parakeet, in English only. Large-v3-turbo is the
measured fallback if a video runs over budget. Kyutai, Canary and Granite were not measured, at the
owner's choice.

**Consequences:** CrispASR builds from its pinned git tag with cmake and nvcc 13.4. Its session API
gives Whisper no initial prompt, so the glossary reaches only the language model. Whisper's
`*sound*` tags reach the diff sheet as words.

**Supersedes:** none.

### 2026-09-26 — The aligner is our CTC Viterbi over Parakeet-CTC

**Context:** Re-timing Parakeet's words on the RoFormer vocal stem, our CTC Viterbi over
Parakeet-CTC-0.6B (fp32) timed every word with 1.7 % more than 200 ms from Parakeet-TDT's times and
no collapsed runs; the Qwen3 forced aligner left 17.2 % over 200 ms and 23 runs of zero-length
words. Both ran in under 8 s. The fp16 Parakeet-CTC export returned NaN for all real audio.

**Decision:** Forced alignment uses our CTC Viterbi over Parakeet-CTC-0.6B fp32 (`model.onnx` with
its data), on the vocal stem, with the spoken-form conversion and the collapsed-run check. When a
block fails, the fallback is the backbone's own word times; the Qwen3 aligner is not used.

**Consequences:** Timing resolution is the model's 80 ms frame; the frame snapping of the cue stage
works on 41.7 ms frames at 24 fps, so finer onsets come from the cue rules, not the aligner.

**Supersedes:** none.

### 2026-09-26 — Sound events come from CED-base through our own ONNX runner

**Context:** The `soundevents` crate forces `ort`'s default features, which pull OpenSSL into the
build, and ships only CED-tiny. CED-base's ONNX export takes raw 16 kHz audio and ends in its own
sigmoid. It scored both stems of Dressrosa 11 in 16.8 s (1.1 min per 120 min, 1.2 GB VRAM).

**Decision:** Sound events use CED-base through `inference::onnx::ced`, 2 s windows every 0.5 s,
three-window median smoothing and per-class thresholds and minimum lengths, with class names
checked against the rated AudioSet table of `soundevents-dataset`.

**Consequences:** Laughter and Singing did not fire on this episode and groans and sighs fired
often; thresholds are tuned in M1 against the owner's viewing, and the sound-events stage gets its
own row in the performance budget.

**Supersedes:** none.

### 2026-09-26 — Shot changes are scanned on the CPU and kept with their scores

**Context:** FFmpeg's scdet over a 480-pixel copy took 19.7 s on the CPU and 57.7 s with NVDEC
decoding, finding the same 1280 changes at score 10; 784 of them lie within 0.5 s of another
(flashes, impact frames). 409 changes score 20 or more.

**Decision:** The scan decodes on the CPU, reports every change scoring 10 or more, and keeps each
score in `shots.json`. The cue stage chooses the score that counts as a cut.

**Consequences:** The shot-change output type carries scores (`ShotCut`); tuning the cut threshold
needs no rescan.

**Supersedes:** none.

### 2026-09-26 — `claude -p` is the default language-model backend

**Context:** On the Dressrosa 11 diff sheet (457 utterances), `claude -p` with Sonnet, eight
processes at once, answered in 38 s (2.5 min per 120 min): every id back, no agreed word dropped,
three novel words (names the glossary lacked), the opening song flagged as lyrics. Qwen3.5-4B
through mistral.rs took 219 s (14.2 min, over the 4-minute budget), used 5.45 GB of VRAM, flagged
nothing, and dropped seven agreed words by cutting a line in half.

**Decision:** Adjudication defaults to `claude -p --model sonnet` with a JSON schema, no tools, only
project settings, in an empty folder, eight processes at once. The local mistral.rs backend stays
available behind its feature as an offline fallback, not the default.

**Consequences:** Adjudication needs the owner's Claude login and network; a run on Dressrosa 11
costs about $0.46 at list price against the subscription. The checks (ids, novel words, dropped
agreed words, reading speed) run on every answer whichever backend gave it.

**Supersedes:** none.

### 2026-09-26 — mistral.rs builds with the CUDA 13.3 compiler

**Context:** mistral.rs v0.9.4, and master, refuse to build with CUDA toolkits above 13.3; the
runtime folder holds 13.4. Qwen3.5 GGUF loading is only on the git tag.

**Decision:** The model store also pins NVIDIA's 13.3.1 compiler pieces (nvcc, crt, libnvvm,
cudart, cccl) into `runtime/cuda-13.3-build/`. mistral.rs builds with them and links against the
13.4 cuBLAS, cuRAND and NVRTC through `LIBRARY_PATH`; it runs on the 13.4 runtime. `mistralrs` is a
git dependency pinned to `v0.9.4`.

**Consequences:** Two toolkits live in the runtime folder, and each build uses one (the
development environment runbook). The workspace lockfile pins `regex` below 1.13 for mistral.rs's
`serde-saphyr`.

**Supersedes:** none.

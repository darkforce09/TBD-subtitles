**Status:** live

# Decisions: stack and pipeline

The decisions the stack spike and the pipeline led to: the runtimes, models and binaries, the
steps and their rules, and the pilot. The [decision log](/documentation/decisions/) says how
entries are written.

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

### 2026-09-26 — The app ships two binaries: `tbd-subtitles` and `tbd-subtitles-ggml`

**Context:** ONNX Runtime, ggml and candle must never share a binary. The pipeline needs ONNX
Runtime for separation, Parakeet, CTC alignment and CED, and ggml for Whisper. The local
language model (candle, through mistral.rs) is not run by the app.

**Decision:** `tbd-subtitles` holds the window, the command line, the job runner, and the workers
for the ONNX Runtime, FFmpeg and `claude` steps. ONNX Runtime is loaded only by those workers.
`tbd-subtitles-ggml` (`apps/tbd_subtitles_ggml`, feature `crispasr`) holds the Whisper steps
only. Both take `worker <step> <job dir>`, and the runner finds the second beside the first.

**Consequences:** The ggml binary builds under the CUDA 13.4 toolkit, as the spike's did.
`cargo build --workspace` never links CrispASR, because the feature is off by default. A third
binary for the local model can join the same way.

**Supersedes:** none.

### 2026-09-26 — Stages run as fingerprinted steps

**Context:** Speech recognition has two engines, and adjudication has a first pass, a re-decode
per engine, a second pass and the choice of sound cues. Each of these must resume and be timed
on its own. Tuning one setting, such as the cut score, should not redo the GPU work.

**Decision:** The runner runs 17 steps (`StepName`), each with one output and one report row. A
step's fingerprint hashes its name and code revision, the settings it reads, the video's path,
size and time (for steps that read the video), and the fingerprint and finish time of each step it
reads. A step is reused when `job.json` holds that fingerprint and its outputs exist;
`--rerun <step>` forces one.

**Consequences:** Changing the cut score reruns only cues, QC and output. Any step that runs
again reruns every step after it. A step whose code changes what it writes must raise its
revision in `crates/pipeline/src/graph/mod.rs`.

**Supersedes:** none.

### 2026-09-26 — Unsure utterances are heard again on the vocal stem by both engines

**Context:** The pipeline re-decodes `UNSURE` spans with alternatives. CrispASR gives Whisper no
initial prompt, so neighbouring lines cannot steer it.

**Decision:** Each unsure utterance, padded by 0.5 s, is transcribed again by Parakeet and by
Whisper on the RoFormer vocal stem. Both hypotheses join the sheet as `ALT p/w`. Only those ids
are asked again, with the settled lines around them as context, and the answer replaces the
first pass for those ids only.

**Consequences:** When nothing is unsure, no model loads and no call is made. The checks run on
the merged answer, so words heard again count as heard.

**Supersedes:** none.

### 2026-09-26 — A shot cut is an scdet change scoring 20 or more

**Context:** The scan keeps every change scoring 10 or more. At 10, 784 of 1280 changes on
Dressrosa 11 lie within 0.5 s of another; at 20, 409 remain.

**Decision:** For cue timing, a cut is a change scoring at least 20 (`--cut-score`). Changes
closer than 0.5 s are merged into the strongest.

**Consequences:** Tuning the score reruns only cues, QC and output.

**Supersedes:** none.

### 2026-09-26 — Replaced subtitle files are kept in the job's work directory

**Context:** An existing subtitle file is backed up before it is replaced; VLC loads one subtitle
file per video, and the media folder is kept clean.

**Decision:** A different existing file is copied to the job's `backup/` folder as
`<file name>.<unix time>`; the new file is written to a part file and renamed. An identical file
is left alone.

**Consequences:** No backup file ever sits beside a video.

**Supersedes:** none.

### 2026-09-26 — The One Piece glossary is built in and used by default

**Context:** The glossary reaches the language model and the novelty check. The first videos are
all One Piece, and a file-manager entry passes no options.

**Decision:** The spike's 56-name glossary lives in
`crates/stages/src/adjudication/glossary/one_piece.json` and is used unless
`--glossary none` or `--glossary <file>` (a JSON array of names) is given.

**Consequences:** A video of another series needs `--glossary`, until the settings file lets the
owner choose per folder.

**Supersedes:** none.

### 2026-09-26 — The offset guard is a signed median

**Context:** The success criteria ask for an aligner-to-engine median difference under 30 ms;
both time words on an 80 ms grid, so the absolute median is one frame whatever the quality.

**Decision:** The job's offset is the signed median of aligner start minus backbone start, over
words both timed in blocks that passed. The quality check reports it when it is 30 ms or more.

**Consequences:** A systematic shift shows; the grid's own scatter does not.

**Supersedes:** none.

### 2026-09-26 — The pipeline milestone stays small

**Context:** The pipeline and style documents name Silero for borderline voice frames, CLAP for
sounds AudioSet lacks, reference subtitles as meaning hints, and speaker labels for off-screen
voices. The owner asked to keep the pipeline milestone small.

**Decision:** The pipeline milestone builds none of them, and none of the local language model's
app worker either. They are listed as later roadmap items, by the owner's word.

**Consequences:** Voice activity is earshot alone; sound cues come from CED and Whisper's tags;
the model sees only the sheet and the glossary; `claude -p` is the only app backend.

**Supersedes:** none.

### 2026-09-26 — The language model marks speaker changes between utterances

**Context:** On the first Dressrosa 11 run the model marked only 2 speaker changes, all inside
utterances with `||`. Quick exchanges split across utterances therefore never shared a
two-speaker cue, and 11 cues came out under 20 frames.

**Decision:** The adjudication rules add the flag `SPK` (the utterance starts with a different
speaker than the one before, judged from sense). A cue too short or too fast alone shares a cue
with its neighbour: dashed when a change was marked, as one speaker's cue when not and the words
fit. A cramped cue may take back the lead-out of the cue before, down to its speech and minimum.

**Consequences:** On the pilot, the model flagged 194 utterances `SPK`, 8 cues became two-speaker
cues, and no cue is under 20 frames. The flag comes from the text alone, so a wrong guess shows as
a wrong dash, not as lost words.

**Supersedes:** none.

### 2026-09-26 — Speech with no cue is measured on the backbone's words

**Context:** The first pilot run reported 215 s of detected speech with no cue. Voice activity on
the vocal stem also fires on grunts, crowds and shouts. Whisper stretches word ends over the
silence that follows, and it writes laughs ("ha ha ha") that the model rightly drops.

**Decision:** The quality check's uncovered speech is made of stretches where Parakeet, the
backbone, heard words, each word counted for at most 1 s, outside the cues, songs and dropped
lines. The voice activity with no cue is reported beside it for reference.

**Consequences:** On the pilot: no heard speech without a cue; 215 s of voice (grunts, crowds)
without one. Speech only Whisper heard, if the model kept it, is covered by its utterance's cue.

**Supersedes:** none.

### 2026-09-26 — The owner accepts the pilot; the batch runs from the GUI

**Context:** The owner watched the Dressrosa 11 pilot in VLC and found the subtitles right. The
pipeline milestone had the batch of 12–48 run from the command line after that.

**Decision:** The pilot is accepted. The batch of 12–48 moves to the GUI milestone, run through
the window's job queue, at the owner's word.

**Consequences:** The batch also tests the queue, progress and report views on 37 episodes. The
pipeline milestone keeps only its 120-minute test.

**Supersedes:** 2026-09-25 — Dressrosa 11 is the pilot; 12–48 are the first batch (when and
where the batch runs).

### 2026-09-28 — Ship as one AppImage bundling CUDA, cuDNN, ONNX Runtime and FFmpeg

**Context:** The owner wants one file to drop into Gear Lever and run, with no dev checkout, no
`target/`, and no host CUDA setup beyond the NVIDIA driver. The CUDA locator already looks in
`<exe_dir>/cuda/{cuda-13.4,cudnn-9.26,onnxruntime-1.28.2}/lib` before the user runtime folder, and
a no-arg launch already opens the GUI, so an AppImage only needs to fill that folder and give the
app its own FFmpeg.

**Decision:** `cargo appimage` builds one AppImage whose `AppDir` carries `usr/bin/cuda/` with the
same `cuda-13.4`/`cudnn-9.26`/`onnxruntime-1.28.2` layout the exe-relative override already reads,
and `usr/bin/ffmpeg/` with a static, pinned FFmpeg 8.1 build. The host supplies only the NVIDIA
driver, X11/EGL, PulseAudio, the desktop portal and FUSE; everything else the app needs to run is
inside the image. The NVIDIA redistributable libraries are bundled for the owner's personal use
only; the AppImage is not published.

**Consequences:** The image is self-contained (verified in the [AppImage
runbook](/documentation/runbooks/building_the_appimage.md)) at the cost of its size, roughly
1.5 GB, dominated by cuDNN. Moving to a new CUDA or ONNX Runtime version means re-pinning the
runtime archives the builder downloads, the same pins `cuda_runtime` already uses.

**Supersedes:** none.

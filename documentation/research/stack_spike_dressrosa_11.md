**Status:** frozen record (2026-09-26)

# Stack spike on Dressrosa 11

Every piece of the [Rust ML stack](/documentation/research/rust_ml_stack.md) run from Rust on one
real episode, `[Muhn Pace] Dressrosa 11.mp4`, on 2026-09-26. The episode is 30.9 minutes long
(1853.7 s, H.264 1080p at 24 fps, one AAC 48 kHz stereo English track).

**How it was measured.** Each item ran in a worker process of its own through the `stack-spike`
tool, which is described in [its README](/tools/stack_spike/README.md):
- **Machine:** an RTX 3070 8 GB (driver 615.71, about 1.4 GB in use by the desktop), an i7-14700K
  and 31 GB of RAM, on the Bazzite host with nothing else running.
- **Wall and load time:** measured around the worker and inside it.
- **Speed:** "× realtime" is video seconds per processing second.
- **Peak VRAM:** NVML sampled the worker's own allocation every 100 ms.
- **Peak RAM:** the worker's `VmHWM`, and the largest child's `ru_maxrss` for FFmpeg.
- **The 120-minute projection:** load time once, plus processing time scaled by 120 / 30.9.

There is no human reference transcript. Every accuracy figure below is therefore a disagreement
between engines, or a proxy, and it says so. "(C++)" marks a Rust crate that binds a C or C++
runtime. The decisions these findings led to are in the [decision log](/documentation/decisions.md).

## Results

| Item | Wall s | Load s | Process s | × realtime | Peak VRAM MiB | Peak RAM MiB | 120 min, min |
|---|---|---|---|---|---|---|---|
| decode: ffprobe, 16 kHz mono to file, 44.1 kHz stereo pass | 1.9 | 0.2 | 1.7 | 1060 | 0 | 6 (FFmpeg 76) | 0.1 |
| shots: scdet on the CPU | 77.4 (with the NVDEC run) | 0 | 19.7 | 94 | 0 | FFmpeg 358 | 1.3 |
| separate-mdx: MDX-Net Voc_FT, batch 1 | 53.2 | 0.3 | 52.7 | 35.2 | 2356 | 1172 | 3.4 |
| separate-roformer: Mel-Band RoFormer | 101.0 | 0.9 | 99.8 | 18.6 | 4280 | 1156 | 6.5 |
| vad: earshot, one stem | 1.7 | 0 | 0.6 | 3289 | 0 | 8 | 0.0 |
| asr-parakeet (mix) | 8.5 | 1.6 | 6.7 | 277 | 3402 | 1206 | 0.5 |
| asr-whisper large-v3 (mix) | 75.1 | 1.6 | 73.4 | 25.3 | 4412 | 628 | 4.8 |
| asr-whisper large-v3-turbo q8 (mix) | 19.6 | 0.8 | 18.7 | 99.2 | 1352 | 604 | 1.2 |
| align-ctc: Viterbi over Parakeet-CTC fp32 | 7.5 | 1.4 | 5.9 | 316 | 3252 | 1202 | 0.4 |
| align-qwen3: Qwen3-ForcedAligner q8 | 7.6 | 0 | 7.5 | 246 | 3432 | 1368 | 0.5 |
| sound-events: CED-base, two stems | 25.6 | 0.3 | 16.8 | 110 | 1200 | 1135 | 1.1 |
| llm-claude: `claude -p` sonnet, 8 processes | 38.1 | 0 | 38.0 | 48.7 | 0 | 268 (CLI) | 2.5 |
| llm-local: Qwen3.5-4B Q4_K_M through mistral.rs | 221.2 | 2.2 | 218.7 | 8.5 | 5454 | 3812 | 14.2 |

**Where the time and memory went:**
- The cold-cache decode, the first read of the 617 MiB file from its hard disk, took 5.9 s.
- Every GPU item stayed within the 5.5 GB VRAM budget. The local language model came closest,
  at 5454 MiB.
- Every worker stayed far under the 8 GB RAM budget.
- MDX-Net at batch 4 ran no faster (55 s) and grew ONNX Runtime's arena to 5426 MiB.
- Speech recognition hears only the 80 chunks of the chunk plan (1591 s of the 1854 s). The speeds
  above are nevertheless counted against the whole video.

## 1. Decoding, probing and shot changes

- **Probe and decode:** ffprobe's JSON gave one untagged audio track, taken as the English one.
  FFmpeg 8.1 streamed 16 kHz mono (113 MiB of `f32`) and 44.1 kHz stereo in one-second chunks
  through a four-chunk channel.
- **Shot scan on NVDEC vs the CPU:** with NVDEC decoding the scdet scan took 57.7 s, because the
  frames are copied back and scaled on the CPU anyway. Plain CPU decoding took 19.7 s and found the
  same 1280 changes.
- **Threshold 10 over-detects:** 784 of the 1280 changes fall within 0.5 s of the previous one
  (flashes, impact frames, fast pans). The count per score is 729 at 15, 409 at 20 (98 of them
  close together), 205 at 25 and 93 at 30.

## 2. Vocal separation

| Separator | 120 min | VRAM | Mean Music score left in the vocal stem after the opening |
|---|---|---|---|
| [UVR MDX-Net Voc_FT](https://huggingface.co/Politrees/UVR_resources) (ort, C++) | 3.4 min | 2.4 GB | 0.041 |
| [Mel-Band RoFormer](https://huggingface.co/silverdaw/mel-band-roformer-vocals-onnx) (ort, C++) | 6.5 min | 4.3 GB | 0.030 |

**How the separators ran:**
- Both run through our own STFT (realfft) and a streaming weighted overlap-add.
- MDX-Net ran as UVR does: 7680-point STFT, 3072 × 256 spectra, `n_fft / 2` edges trimmed,
  compensation 1.021.
- RoFormer used 11 s windows with a Hamming weight and an 8 s step, as its export recommends.
- The stems were written as 16 kHz mono through a windowed-sinc resampler. The background stem is
  the mix minus the vocals.

**Quality proxies** (no reference stems exist):
- *Music left in the vocal stem* (CED's Music class, mean over the windows after the opening):
  RoFormer leaves about a quarter less than MDX-Net.
- *Parakeet's words on each stem against Parakeet on the mix:* 4.0 % disagreement on MDX-Net and
  5.3 % on RoFormer. Neither tells which one is right.
- *Words transcribed during the opening song (0–148 s):* 339 on the mix, 344 on MDX-Net, 307 on
  RoFormer.
- *Listening:* 30 s WAV excerpts at 60 s (song), 600 s and 1200 s of the mix and of both
  separators' stems are in the spike work folder, for the owner's ears.

## 3. Voice activity and chunk plan

[earshot](https://github.com/pykeio/earshot) 1.2.2 (pure Rust) scored every 16 ms frame. The
regions were padded by 200 ms and gaps under 300 ms merged. The chunks run 20–60 s, are cut at
silences of 0.35 s, and always end at a pause of 3 s.

| Input | Speech share | Regions | Chunks | Speech inside the opening song |
|---|---|---|---|---|
| mix | 83 % | 237 | 71 | 146 of 148 s |
| MDX-Net vocals | 74 % | 284 | 88 | 137 s |
| RoFormer vocals | 75 % | 274 | 80 | 137 s |

Every input reads the sung opening as speech. The sound events (singing, music) and the language
model's LYRIC flag are what catch it.

## 4. Speech recognition

| Engine | Words | Disagreement with Parakeet on the mix | Words in the opening song |
|---|---|---|---|
| [Parakeet-TDT-0.6B-v2](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v2-onnx), parakeet-rs 0.3.8 (ort, C++), mix | 3033 | — | 339 |
| Parakeet, RoFormer vocals | 3046 | 5.3 % | 307 |
| [Whisper large-v3](https://huggingface.co/ggerganov/whisper.cpp), CrispASR 0.8.37 (ggml, C++), mix | 3262 | 9.9 % | 374 |
| Whisper large-v3, RoFormer vocals | 3251 | 13.0 % | 352 |
| Whisper large-v3-turbo q8_0, mix | 3179 | 8.5 % | 349 |

**Engine output:**
- **Parakeet** writes punctuation as tokens of their own; each is joined to the word before.
- **Whisper** keeps interjections Parakeet drops ("Huh?", "Huh?!"). It also writes sound tags such
  as `*Grunting*` and `*Coughing*` into its text, which the diff sheet carries as extra words.

**Names:**
- Both engines spell Doflamingo, Rebecca, Lucy, Luffy, Riku, Garp, Blackbeard and Colosseum the
  same way.
- Whisper on the RoFormer stem wrote "Luffy" 24 times against 8–9 elsewhere, a repetition to
  distrust.
- A manual read of stretches such as 14:36–14:50 found both engines faithful to the dub.

**Setup constraints:**
- CrispASR's session API gives Whisper no initial prompt, so the series glossary cannot be handed
  to it.
- CrispASR's crates.io package ships no C++ sources, so the pinned git tag is built with cmake and
  nvcc 13.4, which takes about 3 minutes.

## 5. Forced alignment

Both aligners re-timed Parakeet's mix transcript on the RoFormer vocal stem, chunk by chunk, and
were compared against Parakeet-TDT's own word times.

| Aligner | Words timed | Median start difference vs TDT | Share over 200 ms | Collapsed runs (3 zero-length words) |
|---|---|---|---|---|
| Own CTC Viterbi over [Parakeet-CTC-0.6B fp32](https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX) (ort, C++) | 3033 of 3033 | 80 ms | 1.7 % | 0 |
| [Qwen3-ForcedAligner-0.6B](https://huggingface.co/cstr/qwen3-forced-aligner-0.6b-GGUF) through CrispASR (ggml, C++) | 3028 of 3033 | 80 ms | 17.2 % | 23 |

**How to read the comparison:**
- Both models, and TDT itself, time words on an 80 ms grid, so an 80 ms median is one frame. The
  30 ms goal in the success criteria cannot be read from this comparison.
- The CTC model's greedy decode of the first chunk reads back the opening lyrics correctly: "come
  on we're shining running forever the sea is calling out".
- **Qwen3 failures:** Qwen3 collapsed some words to zero length (for example "but I just" at one
  instant) and misplaced others by about a second.
- **The fp16 CTC export:** the pinned `model_fp16.onnx` returned NaN for every frame of real audio
  on both CPU and CUDA; only an all-zero input survived. The fp32 export has none of this.

## 6. Sound events

[CED-base](https://huggingface.co/mispeech/ced-base) (ort, C++) scored 2 s windows every 0.5 s,
with the rated AudioSet names from `soundevents-dataset` 0.4. The export ends in its own sigmoid:
applying a second sigmoid flattened every class to 0.5.

| Stem | Events (count, seconds) |
|---|---|
| background | Music 76 (1052 s), Explosion 2 (3 s), Gunshot 1 (1 s), Door 1 (2 s) |
| vocals | Groan 33 (59 s), Sigh 30 (42 s), Gasp 12 (20 s), Screaming 6 (8 s), Whimper 3 (4 s), Grunt 1 |

- **Classes not found:** Laughter and Singing never crossed their thresholds. The mean Singing
  score inside the opening song was 0.055, so the song is left to the Music class and the language
  model.
- **Suspect detections:** the vocal-stem groans and sighs are more than the episode sounds like
  and need a look before they become cues.
- **Why not the `soundevents` crate:** it forces `ort`'s default features, including OpenSSL for
  its build-time download.

## 7. Adjudication

The diff sheet built from Parakeet (backbone) and Whisper on the mix held:
- 457 utterances;
- 96 % of the words locked (both engines agreed);
- 78 utterances with a disagreement.

Both backends received the same sheet in batches of 60 with a 56-name glossary.

| Backend | Time (120 min) | Ids back | Novel words | Agreed words dropped | Flags | Cost |
|---|---|---|---|---|---|---|
| `claude -p` sonnet, 8 processes | 38 s (2.5 min) | 457 of 457 | 3 | 0 | LYRIC 32, UNSURE 7, DROP 1 | $0.46 list price (subscription) |
| Qwen3.5-4B Q4_K_M, mistral.rs 0.9.4 | 219 s (14.2 min) | 457 of 457 | 0 | 7 | none | none |

**How the backends differed:**
- **Claude:**
  - It flagged the whole opening song as lyrics and restored glossary spellings: Donquixote,
    Flame-Flame Fruit, Marineford, Scarlett, Ace.
  - Its novel words were names the glossary then lacked, such as "Enies" from Enies Lobby.
- **The local model:**
  - It never flagged the song.
  - It cut an utterance in half: U0206 became "Hey, let go of me!" and lost the rest of the line.
  - It dropped Bartolomeo once. All of this was caught by the checks.
- **The checks:** an earlier check counted "Don Quixote" → "Donquixote" and hyphenated words as
  dropped; joined and hyphenated forms now count as kept.

## 8. Runtimes, builds and process layout

**ONNX Runtime:**
- `ort` 2.0.0-rc.13's prebuilt static ONNX Runtime needs glibc 2.38 and GCC 13's libstdc++ to
  link. The build container has glibc 2.36 and GCC 12.
- So `ort` runs with `load-dynamic`, and Microsoft's `onnxruntime-linux-x64-gpu_cuda13-1.28.2` is
  loaded on the host from the runtime folder.

**CUDA libraries:**
- NVIDIA's redistributable archives (CUDA 13.4.2, cuDNN 9.26) unpack beside it. A GPU worker
  starts with `LD_LIBRARY_PATH` and `ORT_DYLIB_PATH` set.
- nvcc looks for `lib64/`, so a link to `lib/` is made.

**Processes:**
- One process holding two native GPU runtimes corrupts its heap. With CrispASR linked, loading ONNX
  Runtime aborted every time. With mistral.rs linked, three of eight ONNX Runtime workers aborted in
  one run.
- Hence three binaries: `stack-spike` (ONNX Runtime), `stack-spike-ggml` (CrispASR) and
  `stack-spike-llm` (mistral.rs).

**mistral.rs:**
- v0.9.4 and master accept CUDA toolkits up to 13.3. Its kernels therefore build with the 13.3.1
  compiler and link against the 13.4 libraries, in about 30 minutes.
- A crate first built under one toolkit keeps that toolkit's include path in its cached build
  output.
- Qwen3.5 GGUF loads only from the git tag; crates.io 0.8.1 lacks it.

## Projection for a 120-minute video

| Stage | Choice | Projected | Budget | Verdict |
|---|---|---|---|---|
| Decode and shot changes (CPU, alongside the GPU stages) | FFmpeg, scdet on the CPU | 1.4 min | ≤ 3 min | within |
| Vocal separation | Mel-Band RoFormer (MDX-Net 3.4) | 6.5 min | ≤ 12 min | within |
| Voice activity | earshot | 0.0 min | < 1 min | within |
| Speech recognition, backbone | Parakeet-TDT | 0.5 min | ≤ 3 min | within |
| Speech recognition, second engine | Whisper large-v3 (turbo 1.2) | 4.8 min | ≤ 6 min | within |
| Sound events | CED-base | 1.1 min | not budgeted | — |
| Adjudication | `claude -p` (local 14.2) | 2.5 min | ≤ 4 min | within; local misses |
| Forced alignment (cues and QC not yet built) | CTC Viterbi | 0.4 min | ≤ 2 min | within so far |
| **Total**, stages in sequence, decode in parallel | | **≈ 16 min** | ≤ 30 min | within |

The total adds worker start-up of a few seconds per stage.

**What misses:**
- **The local language model** misses its row by more than three times, sits at the VRAM
  ceiling, and does worse in quality.
- **What would fix it:** a smaller or faster local model; fewer utterances per call so the answers
  stay short; or keeping it only as an offline fallback behind `claude -p`.
- **Everything else** fits with room to spare.
- **The re-decode loop** (only the UNSURE spans run again) is not measured yet. Seven utterances
  on this episode would add seconds.

## Hard gaps

- **fp16 Parakeet-CTC export:** returns NaN for real audio on CPU and CUDA; use the fp32 export.
- **Native runtimes in one process:** ggml, candle and ONNX Runtime each need a binary of their
  own.
- **Whisper glossary prompt:** CrispASR's session API has none; whisper-rs would add a second ggml
  binary.
- **mistral.rs and CUDA 13.4:** its build refuses toolkits above 13.3.
- **Prebuilt static ONNX Runtime:** needs a newer glibc and libstdc++ than the build container; the
  runtime-loaded Microsoft build avoids it.
- **Singing:** CED does not find the opening song as Singing; Music, VAD and the LYRIC flag carry
  it.
- **scdet at threshold 10:** over-detects flashes; the cue stage must choose a higher score or
  merge close cuts.

## Sources

[ort 2.0.0-rc.13](https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.13) ·
[ONNX Runtime 1.28.2 release](https://github.com/microsoft/onnxruntime/releases/tag/v1.28.2) ·
[NVIDIA CUDA redistributables](https://developer.download.nvidia.com/compute/cuda/redist/) ·
[CrispASR](https://github.com/CrispStrobe/CrispASR) ·
[mistral.rs](https://github.com/EricLBuehler/mistral.rs) ·
[parakeet-rs](https://github.com/altunenes/parakeet-rs) ·
[earshot](https://github.com/pykeio/earshot) ·
[CED](https://huggingface.co/mispeech/ced-base)

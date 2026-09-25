**Status:** frozen record (2026-09-25)

# Rust ML stack

The Rust crates and ready-made model files that can run each capability the app needs, as found
on 2026-09-25 (crates.io, GitHub, Hugging Face; Claude CLI flags checked against v2.1.282). "(C++)"
marks a Rust crate that binds a C/C++ runtime; see the
[native-runtime decision](/documentation/decisions.md). Nothing here needs Python, and every model
file named is already exported. Versions and dates are from that day.

## 1. Speech recognition

| Option | Version, date | CUDA | Word timing |
|---|---|---|---|
| [parakeet-rs](https://github.com/altunenes/parakeet-rs) (ort, C++) | 0.3.8, 2026-09-23 | yes | TDT tokens → words, 80 ms frames; about 4–5 min of audio per call |
| [transcribe-cpp](https://github.com/handy-computer/transcribe.cpp) (ggml, C++) | 0.2.4, 2026-09-25 | yes | Parakeet tokens without a length cap; Granite-4.1 words |
| [crispasr](https://github.com/CrispStrobe/CrispASR) (ggml, C++) | 0.8.37, 2026-09-25 | yes | native for Whisper, Parakeet, Canary, Cohere, Kyutai |
| [whisper-rs](https://codeberg.org/tazz4843/whisper-rs) (whisper.cpp, C++) | 0.16.0, 2026-03-12 | yes | tokens + DTW, approximate; Silero VAD built in |
| [sherpa-onnx](https://docs.rs/sherpa-onnx) Rust API (C++) | 1.13.8 | CUDA build needed | token times |
| [Kyutai stt-rs](https://github.com/kyutai-labs/delayed-streams-modeling) (candle, pure Rust) | no commits since January | yes | native word times; 2.6B too big for 5.5 GB, 1B en/fr fits |
| [qwen3-asr-rs](https://github.com/alan890104/qwen3-asr-rs) (candle) | — | yes | no timestamps |

Published speeds for Parakeet ONNX: 36× realtime on a 9800X3D CPU, 57× on a T4, 320× on a
5070 Ti with TensorRT. Voxtral Realtime 4B runs in mistral.rs but has no timestamps; Moonshine and
candle's Whisper give no word times.

**Recommendation:** Parakeet-TDT-0.6B-v2 ([istupakov ONNX](https://huggingface.co/istupakov/parakeet-tdt-0.6b-v2-onnx))
through parakeet-rs, fed 60–120 s chunks: about 1–3 min for a 120-minute film. transcribe-cpp with
the [GGUF](https://huggingface.co/handy-computer/parakeet-tdt-0.6b-v2-gguf) avoids ONNX Runtime and
its CUDA 13 need. Second engine for the ensemble: Whisper large-v3, Canary-Qwen or Granite-4.1, in
its own process. parakeet-rs also offers Sortformer speaker diarization.

## 2. Voice activity detection

| Option | Version, date | F1 / cost per second of audio |
|---|---|---|
| [earshot](https://github.com/pykeio/earshot) (pure Rust) | 1.2.2, 2026-08-19 | 0.928 / 0.0003 |
| [silero](https://github.com/Findit-AI/silero) (ort) | 0.7.0, 2026-08-22 | 0.938 / 0.002 |
| [ten-vad-rs](https://github.com/wangfu91/ten-vad-rs) (ort) | 0.1.7, 2026-04-03 | 0.928 / 0.002 |

Benchmark: [wavekat-vad](https://github.com/wavekat/wavekat-vad). Avoid voice_activity_detector
(pinned to an old `ort`) and silero-vad-rs (unmaintained).

**Recommendation:** earshot on the vocal stem, Silero for borderline frames; pad 200 ms, merge
gaps under 300 ms. Every detector fires on songs, so songs are labelled by sound events.

## 3. Vocal separation

| Option | GPU | Notes |
|---|---|---|
| ort + [Mel-Band RoFormer vocals ONNX](https://huggingface.co/silverdaw/mel-band-roformer-vocals-onnx) (MIT, fp16, 707 MB) | CUDA | our STFT: n_fft 2048, hop 441, 44.1 kHz, about 11 s chunks |
| ort + [UVR MDX-Net ONNX](https://huggingface.co/Politrees/UVR_resources) (Voc_FT, Kim_Vocal_2) | CUDA | fastest; STFT and overlap-add with `realfft` |
| [demucs-rs](https://github.com/nikhilunni/demucs-rs) (burn, pure Rust) | Vulkan | htdemucs, htdemucs_ft, 6-stem |
| crispasr `--separate` (C++) | CUDA | Mel-Band RoFormer or htdemucs GGUF |

charon-audio (CPU/CoreML, fast model needs a Python export) and UVR-rs (CPU, about 2× realtime)
fall short. No RTX 3070 figures are published.

**Recommendation:** MDX-Net Voc_FT as the default, Mel-Band RoFormer as a quality mode, fp16 on
CUDA; benchmark first, it is probably the slowest stage.

## 4. Forced alignment

| Option | GPU | Accuracy, notes |
|---|---|---|
| Qwen3-ForcedAligner-0.6B via crispasr `align_words` ([GGUF](https://huggingface.co/cstr/qwen3-forced-aligner-0.6b-GGUF)) (C++) | CUDA | mean error 32 ms vs WhisperX 101 ms, NFA 89 ms; 5 min per call |
| same model, pure Rust: [QwenASR](https://github.com/huanglizhuo/QwenASR), [qwen3-aligner-wgpu](https://github.com/eclipse005/qwen3-aligner-wgpu) | CPU / Vulkan | young projects |
| [asry](https://github.com/findit-studio/asry) 0.2.0 (ort) | CUDA | WhisperX-style wav2vec2 CTC; brand new |
| own CTC Viterbi over [parakeet-ctc-0.6b ONNX](https://huggingface.co/onnx-community/parakeet-ctc-0.6b-ONNX) | CUDA | about 150 lines; we control it fully |

**Recommendation:** Parakeet's own word times for raw recognition; after adjudication, re-align
with our CTC Viterbi, or the Qwen3 aligner where about 30 ms precision matters. Whisper's DTW times
are approximate only.

## 5. Sound events

| Option | Notes |
|---|---|
| [soundevents](https://github.com/findit-studio/soundevents) 0.5.0 (ort) | CED tiny to base ONNX bundled; AudioSet mAP 48.1–50.0; 527 typed labels; chunking built in |
| [AST ONNX](https://huggingface.co/Xenova/ast-finetuned-audioset-10-10-0.4593) + ort | mAP 45.9; needs Kaldi filterbank features |
| [CLAP ONNX](https://huggingface.co/Xenova/larger_clap_general) + ort | zero-shot: labels given as text |
| SenseVoice (sherpa-onnx or crispasr) | emits laughter, applause, crying, cough, music tags inline |

PANNs, BEATs and EfficientAT have no maintained ONNX export; YAMNet is weak.

**Recommendation:** CED-base over 2 s windows every 0.5 s on the mix and the background stem,
mapped to SDH cues with per-class thresholds and smoothing; CLAP for sounds AudioSet lacks.

## 6. Language models

| Option | Status | Notes |
|---|---|---|
| [mistral.rs](https://github.com/EricLBuehler/mistral.rs) (candle, pure Rust) | v0.9.4 on git (crates.io 0.8.1) | CUDA, FlashAttention, GGUF, in-place quantization; Qwen3/3.5/3.6, Gemma 3/4, Qwen3-VL |
| [llama-cpp-2](https://github.com/utilityai/llama-cpp-rs) (C++) | 0.1.157 | `cuda`, `mtmd` (vision), `llguidance` (forced JSON) |

Models that fit about 5.5 GB: Qwen3.5-4B at 4-bit (also reads images), TranslateGemma-4B (Japanese
to English), Qwen3.5-9B at 4-bit (tight).

**Claude CLI as a backend:**
`claude -p --output-format json --json-schema "$SCHEMA" --tools "" --no-session-persistence --strict-mcp-config --disable-slash-commands --system-prompt "$SYS" --model sonnet < batch.txt`.
The answer is in `.structured_output`; `--tools ""` disables all tools; `--bare` would also skip
CLAUDE.md and hooks but works only with an API key, not a subscription login; piped input is capped
at 10 MB.

**Recommendation:** send utterance ids and text only, never timings, and require JSON back.
mistral.rs for local; `claude -p` for best quality on the owner's subscription.

## 7. Japanese on-screen text

| Option | GPU | Notes |
|---|---|---|
| [oar-ocr](https://github.com/GreatV/oar-ocr) 0.9.2 (ort) | CUDA / TensorRT | PP-OCRv5 detection + recognition (Chinese, Japanese, English); models download automatically |
| oar-ocr-vl 0.9.2 (candle) | CUDA, compute 8.0+ (3070 is 8.6) | PaddleOCR-VL-1.6 (0.9B), GLM-OCR |
| manga-ocr ONNX ([l0wgear](https://huggingface.co/l0wgear/manga-ocr-2025-onnx), [mayocream](https://huggingface.co/mayocream/manga-ocr-onnx)) + ort | CUDA | vertical and stylised text crops |
| vision model via mistral.rs or llama-cpp-2 `mtmd` | CUDA | Qwen3.5-4B, Qwen3-VL-4B, PaddleOCR-VL GGUF |

[Koharu](https://github.com/koharu-rs/koharu), a Rust manga translator, is a reference design, not
a library. `ocrs` reads Latin script only.

**Recommendation:** sample frames at 2–4 fps and skip unchanged ones (perceptual hash); detect
text with PP-OCRv5; read it with manga-ocr or PP-OCR; send low-confidence crops to a vision model;
translate with the language model given the surrounding dialogue.

## 8. Shot changes

FFmpeg's `scdet` filter prints `lavfi.scd.time`:
`ffmpeg -hide_banner -nostats -hwaccel cuda -i IN -an -sn -vf scale=480:-2,scdet=threshold=10 -f null -`.
Measured on Dressrosa 08: a 2-minute stretch in 1.5 s on the CPU (about 80× realtime).
[av-scenechange](https://github.com/rust-av/av-scenechange) targets encoder keyframes, not
perceived cuts.

## 9. FFmpeg from Rust

Plain `std::process`, or [ffmpeg-sidecar](https://github.com/nathanbabcock/ffmpeg-sidecar) 2.5.2
(progress parsing). ffmpeg-next links libav and is in maintenance mode. Streaming pattern:
`ffmpeg -nostdin -v error -i IN -map 0:a:m:language:eng -vn -sn -dn -ac 1 -ar 16000 -f f32le pipe:1`,
fixed-size reads into a bounded channel, stderr drained on its own thread, ffprobe JSON for track
choice and `start_time`. 120 minutes of 16 kHz mono f32 is 461 MB; 44.1 kHz stereo is 2.5 GB, so it
is streamed.

## 10. Subtitle files

[rsubs-lib](https://github.com/adracea/rsubs-lib) 0.3.4 (SRT/VTT/SSA),
[subtitler](https://github.com/subtitle-rs/subtitler) 2.8.0, [ass-core](https://github.com/wiedymi/ass-rs)
0.1.2 (ASS parsing); subparse, aspasia, srtlib and libass-rs are unmaintained.
**Recommendation:** our own writer of about 200 lines for SRT, VTT and ASS (SDH brackets, ♪,
`{\an8}`); crates only for importing.

## 11. ONNX Runtime from Rust

[ort](https://github.com/pykeio/ort) 2.0.0-rc.13 wraps ONNX Runtime 1.28. Its prebuilt GPU
binaries are CUDA 13 only (CUDA ≥ 13.2, cuDNN ≥ 9.23, driver ≥ 580; the host has 615). On Bazzite:
the `preload-dylibs` feature with cudart, cuBLAS and cuDNN shipped beside the app, or `load-dynamic`
with a supplied ONNX Runtime build. One `ort` version per binary: crates pinned to older release
candidates (transcribe-rs rc.12, voice_activity_detector rc.10) clash. Alternatives: tract (pure
Rust, CPU only), burn (CUDA/wgpu, partial ONNX import), candle-onnx (limited operators).

## 12. GUI and video preview

egui/eframe 0.36.2, iced 0.14.0, Slint 1.18.1 (GPLv3 or royalty-free licence). Preview:
[egui-sharkplayer](https://codeberg.org/sharkyshark/egui-sharkplayer) (libmpv), or an external mpv
window over JSON IPC. **Recommendation:** eframe plus libmpv; mpv renders ASS through libass and
reloads subtitles after edits.

## Recommended stack

| Capability | Choice |
|---|---|
| Speech recognition | parakeet-rs + Parakeet-TDT-0.6B-v2 ONNX (alternative: transcribe-cpp GGUF) |
| Voice activity | earshot, Silero as backup |
| Vocal separation | ort + MDX-Net / Mel-Band RoFormer ONNX + realfft |
| Forced alignment | own CTC Viterbi over Parakeet-CTC; Qwen3 aligner via crispasr for precision |
| Sound events | soundevents (CED-base) + CLAP ONNX |
| Language model | mistral.rs (Qwen3.5-4B / TranslateGemma-4B); `claude -p` |
| Japanese OCR | oar-ocr + manga-ocr ONNX + a vision model |
| Shot changes | FFmpeg `scdet` |
| FFmpeg | `std::process` or ffmpeg-sidecar |
| Subtitle files | own writer |
| ONNX Runtime | ort rc.13, `cuda` + `preload-dylibs` |
| GUI | eframe + egui-sharkplayer |

## Hard gaps

- **GPU separation:** no maintained Rust crate does it on CUDA; we write the ort code, or use
  crispasr (C++).
- **Qwen3 aligner on CUDA:** only through C++ (crispasr); the pure-Rust ports run on CPU or Vulkan.
- **Frame-level sound-event models:** none exported; CED over sliding windows instead.
- **ggml clashes:** whisper-rs, llama-cpp-2, transcribe-cpp and crispasr each bundle ggml, so no two
  link into one binary; separate worker processes solve it.
- **CUDA 13 libraries on immutable Bazzite:** ship them beside the app.
- **Sung lyrics:** recognition handles singing poorly; mark songs with ♪ instead.

## Sources

[onnx-asr benchmarks](https://github.com/istupakov/onnx-asr) ·
[ort rc.13 release](https://github.com/pykeio/ort/releases/tag/v2.0.0-rc.13) ·
[ort execution providers](https://ort.pyke.io/perf/execution-providers) ·
[CUDA compatibility](https://docs.nvidia.com/deploy/cuda-compatibility/forward-compatibility.html) ·
[CrispASR CLI](https://github.com/CrispStrobe/CrispASR/blob/main/docs/cli.md) ·
[Claude Code headless](https://code.claude.com/docs/en/headless) ·
[Claude Code CLI reference](https://code.claude.com/docs/en/cli-reference) ·
[CED](https://huggingface.co/mispeech/ced-base) ·
[TranslateGemma](https://blog.google/innovation-and-ai/technology/developers-tools/translategemma/) ·
[Qwen3.5 small models](https://artificialanalysis.ai/articles/qwen3-5-small-models) ·
[llama.cpp issue 9267](https://github.com/ggml-org/llama.cpp/issues/9267)

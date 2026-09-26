**Status:** frozen record (2026-09-25)

# Speech recognition landscape

What the speech-recognition field offered on 2026-09-25 for transcribing an English anime dub
with word timing, how the options compared, and why the project runs locally. Numbers were read
from the sources below on that date; recheck them before relying on them.

## The problem

English-dubbed anime is hard for speech recognition: music and effects under the dialogue,
shouting, fast exchanges, overlapping speech and many invented names. Existing subtitles for the
Japanese version do not help directly: the dub's script differs, and the fan edit's cut differs
(tested: One Pace subtitles on Dressrosa ran visibly out of sync with the dub edit).

## Benchmarks

**Artificial Analysis word-error index** (AA-WER; 50 % AgentTalk, 25 % VoxPopuli, 25 % Earnings22,
about 8 hours of audio). Lower is better.

| Model | AA-WER | Price per 1000 min | Weights |
|---|---|---|---|
| Alibaba Fun-Realtime-ASR (preview) | 1.7 % | — | closed |
| Microsoft MAI-Transcribe-2 | 2.0 % | $1.67 | closed |
| ElevenLabs Scribe v2 | 2.2 % | $3.67 | closed |
| Google Gemini 3.5 Transcribe | 2.6 % | $5.00 | closed |
| Mistral Voxtral Small (24B) | 2.8 % | $4.00 | open, too big for 8 GB |
| AssemblyAI Universal-3 Pro | 3.1 % | $3.50 | closed |
| OpenAI GPT Transcribe | 3.3 % | $4.50 | closed |
| Mistral Voxtral Mini Transcribe V2 | 3.6 % | $3.00 | API only (the open sibling is Voxtral Realtime 4B) |
| OpenAI Whisper large-v3 | 4.1 % | — | open |
| NVIDIA Canary-Qwen-2.5B | 4.3 % | — | open |
| Whisper large-v3 turbo | 4.6 % | — | open |
| NVIDIA Parakeet RNNT 1.1B | 5.4 % | — | open |
| NVIDIA Parakeet TDT 0.6B v2 | 6.4 % | — | open |

**Hugging Face Open ASR Leaderboard** (average WER over its English sets): IBM Granite-4.1-2B
5.33, Cohere 5.42, Canary-Qwen-2.5B 5.63, Qwen3-ASR-1.7B 5.76, IBM Granite Speech 3.3 8B 5.85,
Parakeet TDT 0.6B v2 6.05, Parakeet v3 6.32, Kyutai STT 2.6B 6.40, Whisper large-v3 about 7.4.

**Qwen3-ASR-1.7B vs Whisper large-v3** (its report): LibriSpeech other 3.38 vs 3.97, GigaSpeech
8.45 vs 9.76, TED-LIUM 4.50 vs 6.84; claims robustness to music and singing.

The two leaderboards rank the open models differently (Whisper does better on AA-WER than on the
Open ASR Leaderboard), and a 2026 study warns that models are tuned to public benchmarks. None of
the sets is anime or dubbed speech. **Our own audio decides**: the M0.5 spike compares engines on
Dressrosa 08.

## Forced aligners

Qwen3-ForcedAligner-0.6B reports a mean word-boundary error of about 32–43 ms, against about
89–130 ms for NVIDIA's NeMo Forced Aligner and 101–133 ms for WhisperX's wav2vec2 aligner
(figures differ between its report and third-party runs). It handles up to 5 minutes of audio per
call.

## Cloud options (declined)

- ElevenLabs Scribe v2: $0.22 per hour plus $0.05 per hour for up to 1000 prompted key terms; word
  timestamps, speaker labels, audio-event tags. About $5.35 for Dressrosa's 19.8 hours.
- Mistral Voxtral Mini Transcribe V2: $0.003 per minute (about $3.60 for Dressrosa); word
  timestamps, speaker labels, context biasing.
- The owner chose free and local; see [decisions](/documentation/decisions/). A cloud backend
  can be added behind the same interface if local accuracy falls short.

## Anime-specific practice

VoxWeave (MIT, a local subtitle generator aimed at anime) confirms the shape of a good pipeline:
vocal separation with a Mel-Band RoFormer, song detection that keeps openings, endings and insert
songs out of recognition, voice detection chunks of at most 120 s, recognition, then an
"edit and re-align" loop in which corrected text is forced-aligned back to the audio. It is
Python; we take the ideas, not the code.

## Conclusion

A local ensemble: a fast, non-hallucinating backbone (Parakeet TDT) plus one or two engines of a
different design, vocal separation in front, language-model adjudication and re-decoding behind,
and forced alignment for the final timing. The Rust crates that can run each piece are in the
[Rust ML stack](/documentation/research/rust_ml_stack.md).

## Sources

- [Artificial Analysis speech-to-text](https://artificialanalysis.ai/speech-to-text)
- [Hugging Face Open ASR Leaderboard](https://huggingface.co/spaces/hf-audio/open_asr_leaderboard)
- [Qwen3-ASR](https://github.com/QwenLM/Qwen3-ASR) and [Qwen3-ForcedAligner-0.6B](https://huggingface.co/Qwen/Qwen3-ForcedAligner-0.6B)
- [NVIDIA Parakeet TDT 0.6B v2](https://huggingface.co/nvidia/parakeet-tdt-0.6b-v2)
- [ElevenLabs API pricing](https://elevenlabs.io/pricing/api) and [speech-to-text API](https://elevenlabs.io/docs/api-reference/speech-to-text/convert)
- [Voxtral Transcribe 2](https://mistral.ai/news/voxtral-transcribe-2/)
- [Towards Quantifying Benchmark Optimization in ASR Models](https://arxiv.org/pdf/2608.19936)
- [VoxWeave](https://github.com/hali0515/VoxWeave)
- [Best open ASR models in 2026](https://www.marktechpost.com/2026/07/23/best-open-speech-recognition-asr-models-in-2026-wer-languages-latency-and-license-compared/)

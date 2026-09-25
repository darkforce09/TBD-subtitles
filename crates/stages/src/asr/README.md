# Speech recognition stage

Each speech engine over the chunk plan: every chunk cut from a 16 kHz mono file, handed to the
engine, and its words moved from chunk time to video time.

## Contents

```text
crates/stages/src/asr/
├── engines.rs  the `inference` backends as `SpeechEngine`s: Parakeet-TDT, and Whisper with `crispasr`
├── mod.rs      the `SpeechEngine` trait, `transcribe_plan` and the time shift
└── tests/      unit tests for chunk cutting and the shift into video time
```

## How it works

`transcribe_plan` reads each chunk of the plan with `read_f32_range`, asks the engine for its
words, and shifts their times by the chunk's start, clamped inside the chunk. Every engine gets
the same chunks, so the diff sheet can line their words up chunk by chunk. The result is an
`EngineTranscript`: the engine's name, the input it heard, and the words per chunk.

## Boundaries

- Depends on: `media_io::pcm_stream::read_f32_range`; `inference::onnx::parakeet_tdt` and, with the
  `crispasr` feature, `inference::ggml::crispasr`; `job_model::outputs`.
- Used by: `tools/stack_spike/` and `tools/stack_spike_ggml/`.
- Rules: word times are in video seconds and inside their chunk
  (`every_chunk_is_heard_and_timed_in_video_seconds`).

## Related documentation

- [Pipeline](/documentation/architecture/pipeline.md#4-speech-recognition) — the engines and what
  they output.

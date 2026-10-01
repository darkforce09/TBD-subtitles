# PCM audio stream

FFmpeg decoding one audio track to interleaved 32-bit float PCM at 16 kHz mono or 44.1 kHz stereo,
read in fixed-size chunks through a bounded channel, and the raw `.f32` files the decoded mix and
the stems are kept in.

## Contents

```text
crates/media_io/src/pcm_stream/
├── f32_file.rs  raw little-endian `.f32` files: written through `.part` and renamed, read in chunks
├── mod.rs       `PcmStream`: FFmpeg to a pipe, a reader thread, at most four chunks queued
└── tests/       unit tests on a generated tone: rate, chunk sizes, excerpts, files and failures
```

## How it works

```text
PcmStream::open ──▶ ffmpeg -ss? -i video -t? -map 0:a:<n> -ac C -ar R -f f32le pipe:1
                        │ stdout (Run::spawn)            stderr drained by child_process
                        ▼
                 reader thread: fixed chunks ──sync_channel(4)──▶ next_chunk() ──▶ caller
finish(): drain the channel, join the reader, wait FFmpeg ──▶ exit code checked
write_f32_file: every chunk to <path>.part, finish(), rename to <path>
```

A chunk is `chunk_frames × channels` samples; only the last may be shorter. A dropped
`PcmStream` drops its `Running` handle, which kills FFmpeg. `F32FileReader` reads a raw file back
in the same fixed chunks, so a stage streams a stem from the work directory without loading it;
`read_f32_range` reads one window of samples from a sample offset; `F32FileWriter` writes one
sample run at a time and renames its `.part` file into place on `finish`.

## Boundaries

- Depends on: `child_process::Run::spawn` for FFmpeg; `std` for the channel, thread and files.
- Used by:
  - `crates/stages/src/probe_decode/` (the 16 kHz mix) and `crates/stages/src/separation/` (the
    stems), through `PcmStream` and `F32FileWriter`;
  - `crates/stages/src/vad/` through `F32FileReader`, and `crates/stages/src/asr/`,
    `crates/stages/src/sound_events/` and `crates/pipeline/src/tasks/alignment.rs` through
    `read_f32_range`;
  - `tools/stack_spike/` (the decode item writes `mix_16k.f32` with `write_f32_file`) and
    `tools/stack_spike_ggml/` (`read_f32_range`).
- Rules:
  - memory stays bounded: at most `QUEUED_CHUNKS` chunks wait (`decodes_in_fixed_chunks_at_the_asked_rate`);
  - a failed decode is an error at `finish`, never a short file passed as whole
    (`a_missing_file_fails_at_finish`, and the `.part` rename in `write_f32_file`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#9-ffmpeg-from-rust) — the streaming
  pattern and its memory figures.

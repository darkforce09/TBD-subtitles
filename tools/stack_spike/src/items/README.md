# Stack items

The pieces of the ML stack the spike measures, one module each, run inside a worker process.
Each returns the audio seconds it processed, its load and processing times, and its notes. The
ggml items (Whisper, the Qwen3 aligner) are listed here too, but run in `stack-spike-ggml`.

## Contents

```text
tools/stack_spike/src/items/
├── align.rs         CTC Viterbi over Parakeet-CTC on the vocal stem, against the TDT word times
├── asr.rs           Parakeet-TDT over the shared chunk plan on the mix or a stem, and its transcript
├── compare.rs       the transcripts' disagreement with Parakeet on the mix, opening words, name counts
├── decode.rs        ffprobe, 16 kHz mono decoded to `mix_16k.f32`, and a 44.1 kHz stereo pass
├── mod.rs           the `Item` list in run order, which need the GPU or ggml, and their `Outcome`
├── separate.rs      MDX-Net Voc_FT and Mel-Band RoFormer over the track: stems, levels, excerpts
├── shots.rs         the scdet scan on the CPU with cut counts per score, and the NVDEC scan time
├── sound_events.rs  CED-base on both RoFormer stems: events per class, music left in vocal stems
└── vad.rs           earshot and the chunk plan on the mix and both vocal stems, with speech shares
```

## Boundaries

- Depends on: `media_io` (probe, PCM stream, shot scan), `stages::separation`, `stages::vad`,
  `stages::asr`, `stages::alignment`, `stages::sound_events`, `stages::diff_sheet::align`, `inference::onnx`;
  `crate::context::Context` for paths and model files.
- Used by: `tools/stack_spike/src/measure/` (`Item::run` in the worker; `needs_gpu` and `ggml` in
  the parent).
- Rules: an item reads only the video and the work folder, and writes only the work folder (the
  header in `crates/media_io/src/lib.rs` for the video; review for the rest); every speech item
  uses the RoFormer stem's chunk plan (`PLAN` in `asr.rs`), and every aligner re-times the
  Parakeet-on-mix transcript on the RoFormer vocal stem (`TRANSCRIPT` and `AUDIO` in `align.rs`).

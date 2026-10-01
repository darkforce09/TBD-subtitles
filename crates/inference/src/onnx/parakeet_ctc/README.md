# Parakeet-CTC

NVIDIA Parakeet-CTC-0.6B for forced alignment: CTC log-probabilities per 80 ms frame from our
copy of the NeMo log-mel frontend, and the model's own BPE tokens for a spoken word.

## Contents

```text
crates/inference/src/onnx/parakeet_ctc/
├── features.rs  the NeMo log-mel frontend: pre-emphasis, 512-point STFT, 80 Slaney bands, normalised
├── mod.rs       `ParakeetCtc`: the ONNX session, `log_probs`, `tokens`, `greedy_text`
└── tests/       unit tests for the frame count, band normalisation and the filterbank
```

## Boundaries

- Depends on: `crate::onnx::session` (CUDA), `ort`, `realfft`, `tokenizers` (the model's
  `tokenizer.json`).
- Used by: the alignment step in `crates/pipeline/src/tasks/alignment.rs`, which hands the grid
  and tokens to `crates/stages/src/alignment/`; the model list in
  `crates/pipeline/src/models/mod.rs`; and `tools/stack_spike/` (the CTC alignment item).
- Rules:
  - the blank is token 1024 and every grid row is a log-softmax (`log_probs`);
  - the frontend gives `floor(samples / 160)` frames with zero-mean bands
    (`frames_follow_nemo_and_bands_are_normalised`);
  - the model is the fp32 export (`model.onnx` with `model.onnx_data`): the fp16 export returns NaN
    for any real audio on both CPU and CUDA; `greedy_text` and the NaN count in the stack spike's
    notes show which one a run used.

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#4-forced-alignment) — the aligners.

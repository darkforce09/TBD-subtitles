# Separation models

The vocal separation models whose STFT runs outside the ONNX graph: UVR MDX-Net (Voc_FT) and the
host-STFT Mel-Band RoFormer, the STFT that matches the one they were trained with, and the driver
that streams a long stereo track through either with weighted overlap-add.

## Contents

```text
crates/inference/src/onnx/separation/
├── mdx_net.rs       MDX-Net: 7680-point STFT, `[batch, 4, 3072, 256]` spectra, trimmed window edges
├── mel_roformer.rs  Mel-Band RoFormer: 2048-point STFT, `[1, 2050, 1101, 2]`, a complex mask per bin
├── mod.rs           the module list and the re-exports
├── overlap_add.rs   the `WindowModel` trait and the streaming weighted overlap-add over a whole track
├── stft.rs          STFT and inverse STFT as `torch.stft`: centred, reflect-padded, periodic Hann
└── tests/           unit tests for the STFT round trip, the overlap-add coverage and normalisation
```

## How it works

```text
stereo 44.1 kHz chunks ──▶ OverlapAdd::push ──▶ windows at `step` ──▶ WindowModel::separate
                                   ▲                                         │ vocals per window
                                   └── weighted sums ◀── × layout weights ◀──┘
                          frames no later window can change ──▶ Separated { mix, vocals }
```

A `WindowModel` gives its `Layout`: window length, step, the silent frames placed before the
track, and a weight per window frame. MDX-Net windows are `1024 × 255` frames with the
`n_fft / 2` frames at each edge weighted zero and a step that leaves no gap, as UVR trims them;
RoFormer windows are 11 s with a Hamming weight and an 8 s step, as its export recommends. A
frame's vocals are the weight-normalised sum of every window that covered it, and it is handed
out, with its mix sample, once the earliest window not yet run starts after it.

## Boundaries

- Depends on: `ort` (the sessions), `realfft` (the transforms), `crate::onnx::session`.
- Used by: `crates/stages/src/separation/` (the stage driver) and `tools/stack_spike/`.
- Rules:
  - `inverse(forward(x))` returns `x` (`inverse_undoes_forward` in `tests/stft.rs`);
  - every input frame comes out once, in order, and a model that halves its input yields exactly
    half the mix (`trimmed_windows_cover_every_frame_once`,
    `hamming_windows_normalise_their_overlap` in `tests/overlap_add.rs`);
  - each model's input shape is checked against the model file before audio is sent (`open` in
    `mdx_net.rs` and `mel_roformer.rs`).

## Related documentation

- [Rust ML stack](/documentation/research/rust_ml_stack.md#3-vocal-separation) — the separation
  options and their STFT settings.

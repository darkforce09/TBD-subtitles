# LaMa inpainting

LaMa, the large-mask inpainting network, on ONNX Runtime: a 512 × 512 RGB picture and an erase
mask in, the picture with the masked pixels filled from their surroundings out. It fills the
strokes of erased on-screen writing.

## Contents

```text
crates/inference/src/onnx/lama/
├── mod.rs  `Lama::open` (checks the graph's names) and `inpaint`, with the tensor packing
└── tests/  packing and unpacking checks, and a fill of a synthetic gradient on the host's GPU
```

## How it works

`Lama::open` opens `lama_fp32.onnx` from the `lama-inpaint` model folder through `session::open`
and refuses a graph without the inputs `image` and `mask` and the output `output`. `inpaint` takes
interleaved RGB bytes and a mask of the same side, packs the picture as planar channels in 0..1
and the mask as 1 wherever a byte is non-zero, and runs the graph with the picture unmasked: the
graph blanks the masked pixels itself. The output ends in a clip to 0..255, so its values are
rounded and clamped to bytes as they come; on the host a synthetic gradient with black strokes
goes from a mean masked error of 126 levels to 1.9. Pixels outside the mask are returned exactly
as they came in, since the graph's output drifts slightly there.

## Boundaries

- Depends on: `crate::onnx::session` (CUDA), `ort`.
- Used by: `crates/pipeline/src/tasks/replace.rs` (as the stages' `Inpaint` model).
- Rules:
  - inputs are exactly `SIDE` × `SIDE`; fitting a plate to that side is the stage's work;
  - the model check `lama_fills_strokes_from_the_surrounding_gradient` is `#[ignore]` because it
    needs the downloaded model and the host's GPU.

## Related documentation

- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — where
  inpainting sits in in-place text replacement.
- [Inpainting stage](/crates/stages/src/onscreen_text/replace/inpaint/) — how plates are fitted to
  the model's side.

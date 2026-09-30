//! LaMa, the large-mask inpainting network: a 512 × 512 picture and an erase mask in, the picture
//! with the masked pixels filled from their surroundings out.
//!
//! **Role:** fill erased writing strokes in one square context crop at a time.
//! **Position:** used by the on-screen text inpainting stage in its own worker process; runs
//! through `session.rs` on CUDA.
//! **Signals and state:** one ONNX session whose graph takes `image` (RGB in 0..1, planar) and
//! `mask` (1 erases, 0 keeps), blanks the masked pixels itself and returns `output` in 0..255.
//! **Invariants:** inputs are exactly `SIDE` × `SIDE`; pixels outside the mask are returned as
//! they came in.

use std::path::Path;

use ort::session::Session;
use ort::value::Tensor;

use crate::onnx::{Device, OnnxError, session};

/// The model id and folder name.
pub const MODEL: &str = "lama-inpaint";
/// The exported graph in the model folder.
pub const FILE: &str = "lama_fp32.onnx";
/// The fixed input side of the exported graph.
pub const SIDE: usize = 512;

const IMAGE_INPUT: &str = "image";
const MASK_INPUT: &str = "mask";
const OUTPUT: &str = "output";
/// The export ends in a clip to 0..255, so its values are 8-bit pixel levels.
const OUTPUT_LEVELS: f32 = 1.0;

/// An opened LaMa model.
pub struct Lama {
    session: Session,
}

impl Lama {
    /// Open the graph in `dir` and check it has the `image` and `mask` inputs and the `output`.
    pub fn open(dir: &Path, device: Device) -> Result<Lama, OnnxError> {
        let path = dir.join(FILE);
        let session = session::open(&path, device)?;
        let inputs: Vec<&str> = session.inputs().iter().map(|o| o.name()).collect();
        let outputs: Vec<&str> = session.outputs().iter().map(|o| o.name()).collect();
        for (list, name) in [
            (&inputs, IMAGE_INPUT),
            (&inputs, MASK_INPUT),
            (&outputs, OUTPUT),
        ] {
            if !list.contains(&name) {
                return Err(OnnxError::new(
                    format!("opening {}", path.display()),
                    format!(
                        "the graph has no `{name}` (inputs {inputs:?}, outputs {outputs:?}); \
                         expected the LaMa export with `image`, `mask` and `output`"
                    ),
                ));
            }
        }
        Ok(Lama { session })
    }

    /// Fill the masked pixels of `rgb` (`SIDE` × `SIDE` × 3, row-major) where `mask`
    /// (`SIDE` × `SIDE`) is non-zero; returns the same layout.
    pub fn inpaint(&mut self, rgb: &[u8], mask: &[u8]) -> Result<Vec<u8>, OnnxError> {
        check_lengths(rgb, mask)?;
        let image = Tensor::from_array(([1, 3, SIDE, SIDE], planar_image(rgb)))
            .map_err(|e| OnnxError::new("building the LaMa image", e))?;
        let erase = Tensor::from_array(([1, 1, SIDE, SIDE], binary_mask(mask)))
            .map_err(|e| OnnxError::new("building the LaMa mask", e))?;
        let outputs = self
            .session
            .run(ort::inputs![IMAGE_INPUT => image, MASK_INPUT => erase])
            .map_err(|e| OnnxError::new("running LaMa", e))?;
        let (shape, values) = outputs[OUTPUT]
            .try_extract_tensor::<f32>()
            .map_err(|e| OnnxError::new("reading the LaMa output", e))?;
        let expected = [1_i64, 3, SIDE as i64, SIDE as i64];
        if shape[..] != expected[..] {
            return Err(OnnxError::new(
                "reading the LaMa output",
                format!("shape {:?}, expected {expected:?}", &shape[..]),
            ));
        }
        Ok(interleaved_pixels(values, OUTPUT_LEVELS, rgb, mask))
    }
}

/// Both buffers must hold one `SIDE` × `SIDE` picture.
fn check_lengths(rgb: &[u8], mask: &[u8]) -> Result<(), OnnxError> {
    if rgb.len() != SIDE * SIDE * 3 || mask.len() != SIDE * SIDE {
        return Err(OnnxError::new(
            "inpainting with LaMa",
            format!(
                "expected {} RGB bytes and {} mask bytes, got {} and {}",
                SIDE * SIDE * 3,
                SIDE * SIDE,
                rgb.len(),
                mask.len()
            ),
        ));
    }
    Ok(())
}

/// Interleaved RGB bytes as planar channels in 0..1.
fn planar_image(rgb: &[u8]) -> Vec<f32> {
    let mut planar = Vec::with_capacity(rgb.len());
    for channel in 0..3 {
        planar.extend(
            rgb.iter()
                .skip(channel)
                .step_by(3)
                .map(|&v| f32::from(v) / 255.0),
        );
    }
    planar
}

/// Non-zero mask bytes as 1.0, zero as 0.0.
fn binary_mask(mask: &[u8]) -> Vec<f32> {
    mask.iter()
        .map(|&v| if v > 0 { 1.0 } else { 0.0 })
        .collect()
}

/// Planar output channels (`levels` × value gives 0..255) as interleaved bytes, keeping `rgb`
/// exactly wherever `mask` is zero.
fn interleaved_pixels(planar: &[f32], levels: f32, rgb: &[u8], mask: &[u8]) -> Vec<u8> {
    let plane = mask.len();
    let mut out = rgb.to_vec();
    for (pixel, _) in mask.iter().enumerate().filter(|&(_, &m)| m > 0) {
        for channel in 0..3 {
            let value = planar[channel * plane + pixel] * levels;
            out[pixel * 3 + channel] = if value.is_finite() {
                value.round().clamp(0.0, 255.0) as u8
            } else {
                0
            };
        }
    }
    out
}

#[cfg(test)]
#[path = "tests/lama.rs"]
mod tests;

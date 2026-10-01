//! The detector's input: padded frames normalised into one fixed-shape batch.
//!
//! **Role:** check that a job's frames fit a session's input shape, and write them into the
//! session's staging buffer the way the PP-OCRv5 DB detector reads them: each byte scaled by
//! 1/255, ImageNet mean and standard deviation, channels in BGR order, planes in CHW layout.
//!
//! **Position:** called by the pool's session threads before every run, and by the warm-up with
//! no frames at all.
//!
//! **Signals and state:** none; the staging buffer belongs to the session.
//!
//! **Invariants:** the shape never changes between runs: slots after the job's last frame are
//! black frames, and columns right of a frame's width and rows below its height are black; the
//! values equal those of oar-ocr's DB normalisation for the same pixels.

use rayon::prelude::*;

use crate::ocr::OcrError;
use crate::ocr::pool::PaddedFrame;

/// ImageNet statistics in the detector's channel order (blue, green, red), as oar-ocr's DB
/// model hands them to its normaliser.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const STD: [f32; 3] = [0.229, 0.224, 0.225];
const SCALE: f32 = 1.0 / 255.0;
/// The rgb24 byte each input channel reads: blue, green, red.
const SOURCE_CHANNEL: [usize; 3] = [2, 1, 0];

/// One session's input tensor: `[batch, 3, height, width]`, height and width multiples of 32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputShape {
    pub batch: usize,
    pub height: usize,
    pub width: usize,
}

impl InputShape {
    /// The input for `batch` frames of `width` × `height`, both padded to multiples of 32.
    pub fn for_frames(batch: usize, width: u32, height: u32) -> InputShape {
        InputShape {
            batch,
            height: PaddedFrame::padded(height) as usize,
            width: PaddedFrame::padded(width) as usize,
        }
    }

    /// The tensor's dimensions, batch first.
    pub fn dims(self) -> [usize; 4] {
        [self.batch, 3, self.height, self.width]
    }

    /// One channel plane of one image, in values.
    pub fn plane(self) -> usize {
        self.height * self.width
    }

    /// The whole input, in values.
    pub fn len(self) -> usize {
        self.batch * 3 * self.plane()
    }

    /// The probability maps the detector answers with: one plane per image.
    pub fn output_len(self) -> usize {
        self.batch * self.plane()
    }
}

/// Each channel's multiplier and offset: `value = byte × alpha + beta`.
fn coefficients() -> ([f32; 3], [f32; 3]) {
    (
        std::array::from_fn(|c| SCALE / STD[c]),
        std::array::from_fn(|c| -MEAN[c] / STD[c]),
    )
}

/// An error unless `frames` is one to `shape.batch` frames of exactly `width` × `height`, each
/// padded to the shape's height with its rgb24 bytes complete.
pub fn check_frames(
    frames: &[PaddedFrame],
    shape: InputShape,
    width: u32,
    height: u32,
) -> Result<(), OcrError> {
    if frames.is_empty() || frames.len() > shape.batch {
        return Err(format!(
            "a detector job holds {} frames; it needs 1 to {}",
            frames.len(),
            shape.batch
        )
        .into());
    }
    for frame in frames {
        let bytes = frame.width as usize * frame.padded_height as usize * 3;
        if frame.width != width
            || frame.height != height
            || frame.padded_height as usize != shape.height
            || frame.rgb.len() != bytes
        {
            return Err(format!(
                "a {}×{} frame padded to {} rows ({} bytes) does not fit a detector opened for \
                 {width}×{height} frames padded to {} rows",
                frame.width,
                frame.height,
                frame.padded_height,
                frame.rgb.len(),
                shape.height
            )
            .into());
        }
    }
    Ok(())
}

/// Write `frames` into `staging`, laid out as `shape`; every slot after the last frame becomes
/// a black frame. The frames must have passed [`check_frames`].
pub fn normalize_into(
    frames: &[PaddedFrame],
    shape: InputShape,
    staging: &mut [f32],
) -> Result<(), OcrError> {
    if staging.len() != shape.len() || frames.len() > shape.batch {
        return Err("the staging buffer does not match the detector's input".into());
    }
    let (alpha, beta) = coefficients();
    let (height, width) = (shape.height, shape.width);
    staging
        .par_chunks_mut(width)
        .enumerate()
        .for_each(|(row, out)| {
            let image = row / (3 * height);
            let channel = (row / height) % 3;
            let y = row % height;
            let (a, b) = (alpha[channel], beta[channel]);
            let Some(frame) = frames.get(image) else {
                out.fill(b);
                return;
            };
            let frame_width = (frame.width as usize).min(width);
            let start = y * frame.width as usize * 3;
            let source = &frame.rgb[start..start + frame_width * 3];
            let offset = SOURCE_CHANNEL[channel];
            for (value, pixel) in out[..frame_width].iter_mut().zip(source.chunks_exact(3)) {
                *value = f32::from(pixel[offset]) * a + b;
            }
            out[frame_width..].fill(b);
        });
    Ok(())
}

/// Fill `staging` with black frames, as the warm-up runs it.
pub fn fill_black(shape: InputShape, staging: &mut [f32]) -> Result<(), OcrError> {
    normalize_into(&[], shape, staging)
}

#[cfg(test)]
#[path = "tests/batch.rs"]
mod tests;

//! In-place replacement of visible writing: stroke masks, inpainted plates and English lettering
//! composed for the localized video.
//!
//! **Role:** turn each translated occurrence into plates the localized video blends over the
//! source frames, or record why it stays in the subtitle file.
//! **Position:** on-screen text stages after review and before typesetting; the pipeline runs
//! mask extraction and composition on the CPU and inpainting in an ONNX Runtime worker.
//! **Signals and state:** full-resolution region crops decoded on demand, PNG masks, plates and
//! patches in the job's `visual/` folder, and one `ReplacementDocument` per step.
//! **Invariants:** source pixels are read-only; only masked pixels are ever replaced; an
//! occurrence that cannot be separated, tracked or lettered legibly falls back with a reason.

pub mod compose;
pub mod inpaint;
pub mod mask;
pub mod source;
pub mod verify;

use image::RgbImage;
use job_model::onscreen::PixelRect;

use crate::onscreen_text::TextResult;

/// Full-resolution pixels of chosen regions of the source video.
pub trait RegionSource {
    /// Origin-relative (time_s, end_s) per frame index, known before decoding.
    fn timeline(&self) -> &[(f64, f64)];
    /// Source frame size in pixels.
    fn frame_size(&self) -> (u32, u32);
    /// Decode `rect` of frames `first..=last` and hand each to `visit` in order with its index.
    fn frames(
        &mut self,
        rect: PixelRect,
        first: u64,
        last: u64,
        visit: &mut dyn FnMut(u64, RgbImage) -> TextResult<()>,
    ) -> TextResult<()>;
}

/// Whether the occurrence is writing Claude found on a keyframe that the local detector missed:
/// its id ends in `-c` and a number, and its one quad is Claude's loose box.
pub fn found_by_claude(id: &str) -> bool {
    id.rsplit_once("-c").is_some_and(|(stem, number)| {
        !stem.is_empty() && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit())
    })
}

#[cfg(test)]
#[path = "tests/claude_ids.rs"]
mod claude_ids;

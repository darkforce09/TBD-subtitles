//! Patch pixels: warped lettering over the inpainted plate, with the erase mask's coverage.
//!
//! **Role:** build the RGBA patch the localized video blends over the source frames, and the
//! review preview.
//! **Position:** last pixel step of `compose`, before the files are written.
//! **Signals and state:** one plate, its mask and the warped lettering in; one RGBA image out.
//! **Invariants:** RGB is the lettering over the inpainted plate everywhere, so a transparent
//! pixel shows the plate's own colour; alpha covers the feathered mask and the lettering and is
//! zero elsewhere; files appear only through a rename.

use std::path::Path;

use image::{GrayImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};

use crate::onscreen_text::TextResult;

/// The erase mask in 0..1, averaged over each pixel's 3 × 3 neighbourhood inside the image.
pub(crate) fn feather(mask: &GrayImage) -> Vec<f32> {
    let (width, height) = (mask.width() as i64, mask.height() as i64);
    let mut output = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            let (mut sum, mut count) = (0.0f32, 0.0f32);
            for ny in (y - 1).max(0)..=(y + 1).min(height - 1) {
                for nx in (x - 1).max(0)..=(x + 1).min(width - 1) {
                    sum += f32::from(mask.get_pixel(nx as u32, ny as u32).0[0]) / 255.0;
                    count += 1.0;
                }
            }
            output.push(sum / count);
        }
    }
    output
}

/// The patch for one plate: `lettering` is premultiplied RGBA in 0..1 per plate pixel.
pub(crate) fn patch(plate: &RgbImage, mask: &GrayImage, lettering: &[[f32; 4]]) -> RgbaImage {
    let feathered = feather(mask);
    let mut output = RgbaImage::new(plate.width(), plate.height());
    for (index, (x, y, pixel)) in output.enumerate_pixels_mut().enumerate() {
        let letter = lettering.get(index).copied().unwrap_or([0.0; 4]);
        let under = plate.get_pixel(x, y).0;
        let mut rgba = [0u8; 4];
        for channel in 0..3 {
            let value = letter[channel] + f32::from(under[channel]) / 255.0 * (1.0 - letter[3]);
            rgba[channel] = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        let alpha = feathered.get(index).copied().unwrap_or(0.0).max(letter[3]);
        rgba[3] = (alpha.clamp(0.0, 1.0) * 255.0).round() as u8;
        *pixel = Rgba(rgba);
    }
    output
}

/// The patch blended over the original pixels it replaces.
pub(crate) fn preview(patch: &RgbaImage, source: &RgbImage) -> RgbImage {
    let mut output = RgbImage::new(patch.width(), patch.height());
    for (x, y, pixel) in output.enumerate_pixels_mut() {
        let [r, g, b, a] = patch.get_pixel(x, y).0;
        let under = source.get_pixel(x, y).0;
        let alpha = f32::from(a) / 255.0;
        let blend = |top: u8, bottom: u8| {
            (f32::from(top) * alpha + f32::from(bottom) * (1.0 - alpha)).round() as u8
        };
        *pixel = Rgb([blend(r, under[0]), blend(g, under[1]), blend(b, under[2])]);
    }
    output
}

/// Write a PNG beside its final name, then rename it into place.
pub(crate) fn write_png(path: &Path, image: &image::DynamicImage) -> TextResult<()> {
    let temporary = path.with_extension("png.tmp");
    image
        .save_with_format(&temporary, ImageFormat::Png)
        .map_err(|e| format!("Cannot write {}: {e}", temporary.display()))?;
    std::fs::rename(&temporary, path)
        .map_err(|e| format!("Cannot move {} into place: {e}", path.display()))?;
    Ok(())
}

#[cfg(test)]
#[path = "tests/patch.rs"]
mod tests;

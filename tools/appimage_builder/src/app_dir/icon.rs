//! The application icon as an RGBA PNG: `app_icon`'s pixels, encoded with the `png` crate.

use anyhow::{Context, Result};

/// The icon as a `size`×`size` RGBA PNG.
pub(crate) fn icon_png(size: u32) -> Result<Vec<u8>> {
    let pixels = app_icon::rgba(size);
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, size, size);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .context("writing the icon's PNG header")?;
    writer
        .write_image_data(&pixels)
        .context("writing the icon's pixels")?;
    writer.finish().context("finishing the icon PNG")?;
    Ok(out)
}

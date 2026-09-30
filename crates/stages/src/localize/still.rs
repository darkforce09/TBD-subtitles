//! One region of one frame as the localized video shows it, back in RGB for inspection.
//!
//! **Role:** blend the patches that cover a frame over a region of that frame exactly as the
//! render loop blends them, in 8-bit 4:2:0 samples of the stream's colour conversion, and
//! convert the result back to R′G′B′ for a reader that looks at the finished picture.
//! **Position:** used by the read-back check of lettered replacements
//! (`onscreen_text::replace::verify`); reuses `blend::convert` and `blend::blend` and the
//! inverse of `colour::Conversion`.
//! **Signals and state:** pure functions of one region, its patches and the conversion.
//! **Invariants:** the region starts on even coordinates and has even sides, so its 2×2 chroma
//! blocks are the frame's own; a patch contributes only the part of it inside the region; patches
//! stack in the order given; a region no patch touches round-trips through the conversion alone.

use image::{RgbImage, RgbaImage, imageops};
use job_model::onscreen::PixelRect;
use media_io::video_frames::PixelFormat;

use super::LocalizeResult;
use super::blend::{blend, convert};
use super::colour::Conversion;

/// A composed patch and where it sits in the frame.
#[derive(Debug, Clone, Copy)]
pub struct PlacedPatch<'a> {
    pub image: &'a RgbaImage,
    pub rect: PixelRect,
}

/// The region of a `width` × `height` frame covering `rect` that starts on even coordinates and
/// has even sides, inside the frame; none when nothing of `rect` fits.
pub fn even_region(rect: PixelRect, (width, height): (u32, u32)) -> Option<PixelRect> {
    let (frame_right, frame_bottom) = (width & !1, height & !1);
    let x = (rect.x & !1).min(frame_right);
    let y = (rect.y & !1).min(frame_bottom);
    let right = (rect.right().saturating_add(1) & !1).min(frame_right);
    let bottom = (rect.bottom().saturating_add(1) & !1).min(frame_bottom);
    (right > x && bottom > y).then(|| PixelRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

/// `crop`, the pixels of `region` of a source frame, with every patch of `patches` blended over
/// it in order as the localized video blends them under `conversion` at 8 bits.
pub fn finished_region(
    crop: &RgbImage,
    region: PixelRect,
    patches: &[PlacedPatch],
    conversion: Conversion,
) -> LocalizeResult<RgbImage> {
    if crop.dimensions() != (region.width, region.height) {
        return Err("a region crop differs from its region's size".into());
    }
    if [region.x, region.y, region.width, region.height]
        .iter()
        .any(|value| !value.is_multiple_of(2))
    {
        return Err("a finished region must start and end on even pixels".into());
    }
    let conversion = Conversion {
        bits: 8,
        ..conversion
    };
    let size = (region.width, region.height);
    let mut frame = samples(crop, conversion);
    for patch in patches {
        let Some(inside) = intersection(patch.rect, region) else {
            continue;
        };
        let part = imageops::crop_imm(
            patch.image,
            inside.x - patch.rect.x,
            inside.y - patch.rect.y,
            inside.width,
            inside.height,
        )
        .to_image();
        let local = PixelRect {
            x: inside.x - region.x,
            y: inside.y - region.y,
            ..inside
        };
        blend(
            &mut frame,
            PixelFormat::Yuv420p,
            size,
            &convert(&part, local, conversion)?,
        )?;
    }
    Ok(pixels(&frame, size, conversion))
}

/// The pixels two rectangles share, when they share any.
fn intersection(a: PixelRect, b: PixelRect) -> Option<PixelRect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let right = a.right().min(b.right());
    let bottom = a.bottom().min(b.bottom());
    (right > x && bottom > y).then(|| PixelRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}

/// An 8-bit 4:2:0 frame of `crop`: luma per pixel, Cb and Cr per 2×2 block as the mean of its
/// pixels', rounded.
fn samples(crop: &RgbImage, conversion: Conversion) -> Vec<u8> {
    let (width, height) = crop.dimensions();
    let (w, h) = (width as usize, height as usize);
    let mut frame = vec![0u8; w * h + 2 * (w / 2) * (h / 2)];
    let chroma = w * h;
    let plane = (w / 2) * (h / 2);
    for by in 0..h / 2 {
        for bx in 0..w / 2 {
            let (mut cb, mut cr) = (0.0, 0.0);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (x, y) = (2 * bx + dx, 2 * by + dy);
                let yuv = conversion.yuv(crop.get_pixel(x as u32, y as u32).0);
                frame[y * w + x] = yuv[0].round() as u8;
                cb += yuv[1];
                cr += yuv[2];
            }
            let block = by * (w / 2) + bx;
            frame[chroma + block] = (cb / 4.0).round() as u8;
            frame[chroma + plane + block] = (cr / 4.0).round() as u8;
        }
    }
    frame
}

/// The R′G′B′ pixels of an 8-bit 4:2:0 frame of `size`.
fn pixels(frame: &[u8], (width, height): (u32, u32), conversion: Conversion) -> RgbImage {
    let (w, h) = (width as usize, height as usize);
    let chroma = w * h;
    let plane = (w / 2) * (h / 2);
    RgbImage::from_fn(width, height, |x, y| {
        let (x, y) = (x as usize, y as usize);
        let block = (y / 2) * (w / 2) + x / 2;
        image::Rgb(conversion.rgb([
            u16::from(frame[y * w + x]),
            u16::from(frame[chroma + block]),
            u16::from(frame[chroma + plane + block]),
        ]))
    })
}

#[cfg(test)]
#[path = "tests/still.rs"]
mod tests;

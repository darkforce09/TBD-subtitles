//! Box images: what full-resolution screening finds on a frame against what the proxy found.
//!
//! **Role:** pick a handful of sample frames spread over the clip, run the stock detector on
//! their 640-wide proxies as the proxy baseline does, and write each frame at full resolution as
//! a PNG with the production pool's CUDA boxes, its TensorRT FP16 boxes and the proxy's boxes,
//! scaled up to the frame, each in its own colour; print a legend and a per-image box count.
//! **Position:** the last section of `detect-bench`, given the CUDA reference run's and TensorRT
//! FP16's regions by the pool sections; the proxy boxes are found earlier, while the proxies are
//! still held.
//! **Signals and state:** one stock detector session while the proxy boxes are found; the PNGs
//! it writes under the folder it is given.
//! **Invariants:** a scaled or drawn corner never leaves the frame; images are written only
//! under the given folder; a failure prints its error and the bench goes on.

use std::path::Path;

use image::{Rgb, RgbImage};
use imageproc::drawing::draw_line_segment_mut;
use inference::ocr::pool::PaddedFrame;

use super::detector::{Quad, Screen, Settings, Stock};
use super::pool_runs::PoolBoxes;
use super::table::Table;

/// The full-resolution CUDA pool's boxes: blue, thick.
const FULL_COLOUR: Rgb<u8> = Rgb([0, 90, 255]);
const FULL_THICKNESS: u32 = 3;
/// The proxy's boxes, drawn over the full-resolution ones.
const PROXY_COLOUR: Rgb<u8> = Rgb([255, 0, 255]);
const PROXY_THICKNESS: u32 = 1;
/// TensorRT FP16's boxes: green, thin, inside the CUDA boxes they agree with.
const FP16_COLOUR: Rgb<u8> = Rgb([0, 230, 64]);
const FP16_THICKNESS: u32 = 1;
/// The box score the proxy keeps, as screening does.
const SCREEN_BOX_SCORE: f32 = 0.3;

/// The proxy's boxes on the picked frames.
pub struct ProxyBoxes {
    pub size: (u32, u32),
    /// Sample indices, ascending.
    pub picks: Vec<usize>,
    /// One box list per pick, in proxy pixels.
    pub boxes: Vec<Vec<Quad>>,
}

/// Where the images go and how a sample index maps to a time in the video.
pub struct Destination<'a> {
    pub dir: &'a Path,
    pub start_s: f64,
    /// Seconds between two samples.
    pub step_s: f64,
}

/// `count` indices spread evenly over `total` samples, from the first; every sample when there
/// are no more than `count`.
pub fn pick(total: usize, count: usize) -> Vec<usize> {
    if count == 0 || total == 0 {
        return Vec::new();
    }
    if total <= count {
        return (0..total).collect();
    }
    let mut picks: Vec<usize> = (0..count).map(|i| i * total / count).collect();
    picks.dedup();
    picks
}

/// `corners` moved into a `size` frame: every coordinate between 0 and the last pixel.
pub fn clip_to(corners: Quad, (width, height): (u32, u32)) -> Quad {
    let right = width.saturating_sub(1) as f32;
    let bottom = height.saturating_sub(1) as f32;
    corners.map(|(x, y)| (x.clamp(0.0, right), y.clamp(0.0, bottom)))
}

/// `corners`, found on a `from` proxy, in the pixels of a `to` frame, clipped into it.
pub fn scale_quad(corners: Quad, from: (u32, u32), to: (u32, u32)) -> Quad {
    let sx = to.0 as f32 / from.0.max(1) as f32;
    let sy = to.1 as f32 / from.1.max(1) as f32;
    clip_to(corners.map(|(x, y)| (x * sx, y * sy)), to)
}

/// A pool region's corners as the drawing takes them.
pub fn corners(quad: &job_model::onscreen::Quad) -> Quad {
    quad.0.map(|point| (point.x as f32, point.y as f32))
}

/// Draw `corners`' four edges on `image`, `thickness` pixels wide.
pub fn draw_quad(image: &mut RgbImage, corners: &Quad, colour: Rgb<u8>, thickness: u32) {
    let reach = (thickness.max(1) as i32 - 1) / 2;
    let spread = || (-reach..=reach).map(|offset| offset as f32);
    for edge in 0..4 {
        let (a, b) = (corners[edge], corners[(edge + 1) % 4]);
        for dx in spread() {
            for dy in spread() {
                let start = (a.0 + dx, a.1 + dy);
                let end = (b.0 + dx, b.1 + dy);
                draw_line_segment_mut(image, start, end, colour);
            }
        }
    }
}

/// The stock detector's boxes, at its default limit and on the environment's provider, on
/// `count` proxies spread over the clip.
pub fn proxy_boxes(
    model: &Path,
    proxies: &[image::RgbImage],
    count: usize,
) -> Result<ProxyBoxes, String> {
    let size = proxies.first().ok_or("no proxy frames")?.dimensions();
    let picks = pick(proxies.len(), count);
    let settings = Settings {
        model,
        limit_side: None,
        box_score: SCREEN_BOX_SCORE,
        memory_limit_mib: None,
    };
    let mut detector = Stock::open(&settings)?;
    let mut boxes = Vec::with_capacity(picks.len());
    for &index in &picks {
        let found = detector.screen(&[&proxies[index]])?;
        let regions = found.into_iter().next().unwrap_or_default();
        boxes.push(regions.into_iter().map(|(quad, _)| quad).collect());
    }
    Ok(ProxyBoxes { size, picks, boxes })
}

/// The section: one PNG per picked frame, its legend and box counts.
pub fn section(
    destination: &Destination<'_>,
    frames: &[PaddedFrame],
    pool: &PoolBoxes,
    proxy: &Result<ProxyBoxes, String>,
) {
    println!("## 9. Box images in {}\n", destination.dir.display());
    println!(
        "{}\n",
        legend(pool.cuda.is_some(), pool.tensorrt_fp16.is_some())
    );
    let proxy = match proxy {
        Ok(proxy) => proxy,
        Err(error) => {
            println!("No images: the proxy detector failed: {error}\n");
            return;
        }
    };
    if let Err(error) = std::fs::create_dir_all(destination.dir) {
        println!("No images: {}: {error}\n", destination.dir.display());
        return;
    }
    let mut table = Table::new(&[
        "Image",
        "Full-resolution boxes",
        "TensorRT FP16 boxes",
        "Proxy boxes",
        "Note",
    ]);
    for (&index, proxy_quads) in proxy.picks.iter().zip(&proxy.boxes) {
        let Some(frame) = frames.get(index) else {
            continue;
        };
        let time_s = destination.start_s + index as f64 * destination.step_s;
        let name = format!("sample_{index:04}_{time_s:.1}s.png");
        let regions = pool.cuda.as_ref().and_then(|regions| regions.get(index));
        let fp16 = pool.tensorrt_fp16.as_ref().and_then(|r| r.get(index));
        let written = write_image(
            &destination.dir.join(&name),
            frame,
            [
                (regions, FULL_COLOUR, FULL_THICKNESS),
                (fp16, FP16_COLOUR, FP16_THICKNESS),
            ],
            proxy_quads,
            proxy.size,
        );
        let count =
            |found: Option<&Vec<_>>| found.map_or("n/a".to_string(), |r| r.len().to_string());
        table.row(vec![
            name,
            count(regions),
            count(fp16),
            proxy_quads.len().to_string(),
            written
                .err()
                .map_or_else(String::new, |e| format!("error: {e}")),
        ]);
    }
    println!("{}", table.render());
}

/// The legend line naming each colour.
pub fn legend(with_full: bool, with_fp16: bool) -> String {
    let full = if with_full {
        "blue, 3 px: the production pool's boxes at full resolution (CUDA reference run)"
    } else {
        "no full-resolution boxes: the CUDA reference run failed"
    };
    let fp16 = if with_fp16 {
        "green, 1 px: the TensorRT FP16 pool's boxes at full resolution"
    } else {
        "no TensorRT FP16 boxes"
    };
    format!(
        "Legend: {full}; {fp16}; magenta, 1 px: the 640-wide proxy's boxes, scaled up to the \
         frame."
    )
}

/// One frame's full-resolution boxes, when the run found them, with their colour and thickness.
type BoxSet<'a> = (
    Option<&'a Vec<(job_model::onscreen::Quad, f64)>>,
    Rgb<u8>,
    u32,
);

/// Write `frame`'s picture to `path` with each full-resolution set of boxes in its colour and
/// thickness, then the proxy's boxes.
fn write_image(
    path: &Path,
    frame: &PaddedFrame,
    full: [BoxSet<'_>; 2],
    proxy: &[Quad],
    proxy_size: (u32, u32),
) -> Result<(), String> {
    let size = (frame.width, frame.height);
    let bytes = frame.width as usize * frame.height as usize * 3;
    let picture = frame.rgb.get(..bytes).ok_or("the frame is short")?.to_vec();
    let mut image = RgbImage::from_raw(size.0, size.1, picture).ok_or("the frame's size")?;
    for (regions, colour, thickness) in full {
        for (quad, _) in regions.into_iter().flatten() {
            let quad = clip_to(corners(quad), size);
            draw_quad(&mut image, &quad, colour, thickness);
        }
    }
    for quad in proxy {
        let quad = scale_quad(*quad, proxy_size, size);
        draw_quad(&mut image, &quad, PROXY_COLOUR, PROXY_THICKNESS);
    }
    image.save(path).map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "tests/overlay.rs"]
mod tests;

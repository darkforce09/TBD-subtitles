//! Inpainting: each pending occurrence's plates with the erased strokes filled from their
//! surroundings.
//!
//! **Role:** read every plate's source crop and erase mask, fill the masked pixels through a
//! square inpainting model, and record the filled plate beside the others.
//! **Position:** the step between stroke masks and lettering composition; the pipeline runs it in
//! the ONNX Runtime worker with LaMa behind the `Inpaint` trait. `fill.rs` fits one plate to the
//! model.
//! **Signals and state:** one plate decoded at a time, and a bounded cache of finished plates
//! keyed by the exact bytes of their source and mask files. Plates go to
//! `visual/plates/<occurrence id>/<plate index>.png`.
//! **Invariants:** only `Pending` occurrences change; a plate's pixels further than one pixel
//! from its mask are its source bytes; a mask without a set pixel costs no model call; a missing
//! or mis-sized file fails the step; every plate file is complete once its path is recorded.

mod fill;

use std::collections::hash_map::DefaultHasher;
use std::collections::{HashSet, VecDeque};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::Path;

use image::{ImageFormat, RgbImage};
use job_model::onscreen::{Plate, ReplaceStatus, ReplacementDocument};

use crate::onscreen_text::TextResult;

/// The folder of filled plates, relative to the job directory.
const PLATES_DIR: &str = "visual/plates";
/// The most finished plates the cache holds.
const CACHE_ENTRIES: usize = 64;
/// The most bytes (keys and pixels) the cache holds.
const CACHE_BYTES: usize = 256 << 20;

/// A square inpainting model with a fixed side.
pub trait Inpaint {
    /// The input side in pixels.
    fn side(&self) -> usize;
    /// Fill the pixels of `rgb` (side × side × 3) where `mask` (side × side) is non-zero.
    fn inpaint(&mut self, rgb: &[u8], mask: &[u8]) -> TextResult<Vec<u8>>;
}

/// Inpaint every plate of every occurrence still pending.
pub fn inpaint(
    document: &mut ReplacementDocument,
    root: &Path,
    model: &mut dyn Inpaint,
    progress: &(dyn Fn(usize, usize) + Sync),
) -> TextResult<()> {
    let total = document
        .texts
        .iter()
        .filter(|text| text.status == ReplaceStatus::Pending)
        .map(|text| text.plates.len())
        .sum();
    progress(0, total);
    let mut cache = PlateCache::default();
    let mut folders = HashSet::new();
    let mut done = 0;
    for text in &mut document.texts {
        if text.status != ReplaceStatus::Pending || text.plates.is_empty() {
            continue;
        }
        let folder = Path::new(PLATES_DIR).join(unique_folder(&text.id, &mut folders));
        fs::create_dir_all(root.join(&folder))
            .map_err(|e| format!("creating {}: {e}", root.join(&folder).display()))?;
        for (index, plate) in text.plates.iter_mut().enumerate() {
            let filled = filled_plate(plate, root, model, &mut cache)
                .map_err(|e| format!("{} plate {index}: {e}", text.id))?;
            let relative = folder.join(format!("{index}.png"));
            write_png(&filled, &root.join(&relative))?;
            plate.plate = Some(relative);
            done += 1;
            progress(done, total);
        }
    }
    Ok(())
}

/// The filled pixels of one plate, from the cache when an identical plate was already filled.
fn filled_plate(
    plate: &Plate,
    root: &Path,
    model: &mut dyn Inpaint,
    cache: &mut PlateCache,
) -> TextResult<RgbImage> {
    let source_bytes = read(root, &plate.source)?;
    let mask_bytes = read(root, &plate.mask)?;
    let expected = (plate.rect.width, plate.rect.height);
    if let Some(filled) = cache.get(&source_bytes, &mask_bytes) {
        if filled.dimensions() != expected {
            return Err(size_error(&plate.source, filled.dimensions(), expected));
        }
        return Ok(filled.clone());
    }
    let source = decode(&source_bytes, &plate.source)?.into_rgb8();
    let mask = decode(&mask_bytes, &plate.mask)?.into_luma8();
    for (path, size) in [
        (&plate.source, source.dimensions()),
        (&plate.mask, mask.dimensions()),
    ] {
        if size != expected {
            return Err(size_error(path, size, expected));
        }
    }
    let filled = if mask.pixels().any(|p| p.0[0] > 0) {
        fill::fill_plate(&source, &mask, model)?
    } else {
        source
    };
    cache.insert(source_bytes, mask_bytes, filled.clone());
    Ok(filled)
}

fn read(root: &Path, relative: &Path) -> TextResult<Vec<u8>> {
    let path = root.join(relative);
    fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()).into())
}

fn decode(bytes: &[u8], path: &Path) -> TextResult<image::DynamicImage> {
    image::load_from_memory_with_format(bytes, ImageFormat::Png)
        .map_err(|e| format!("decoding {}: {e}", path.display()).into())
}

fn size_error(path: &Path, size: (u32, u32), expected: (u32, u32)) -> inference::ocr::OcrError {
    format!(
        "{} is {} × {}, but its plate is {} × {}",
        path.display(),
        size.0,
        size.1,
        expected.0,
        expected.1
    )
    .into()
}

/// Write `image` as a PNG under a temporary name, then move it into place.
fn write_png(image: &RgbImage, path: &Path) -> TextResult<()> {
    let partial = path.with_extension("png.partial");
    image
        .save_with_format(&partial, ImageFormat::Png)
        .map_err(|e| format!("writing {}: {e}", partial.display()))?;
    fs::rename(&partial, path).map_err(|e| format!("moving {} into place: {e}", path.display()))?;
    Ok(())
}

/// A folder name for `id` made of ASCII letters, digits, `-` and `_`, distinct from every name
/// already in `taken`.
fn unique_folder(id: &str, taken: &mut HashSet<String>) -> String {
    let mut base: String = id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if base.is_empty() {
        base.push('_');
    }
    let mut name = base.clone();
    let mut suffix = 2;
    while !taken.insert(name.clone()) {
        name = format!("{base}-{suffix}");
        suffix += 1;
    }
    name
}

/// Finished plates by the exact bytes of their source and mask files, oldest dropped first.
#[derive(Default)]
struct PlateCache {
    entries: VecDeque<CachedPlate>,
    bytes: usize,
}

struct CachedPlate {
    hash: u64,
    source: Vec<u8>,
    mask: Vec<u8>,
    filled: RgbImage,
}

impl CachedPlate {
    fn bytes(&self) -> usize {
        self.source.len() + self.mask.len() + self.filled.as_raw().len()
    }
}

impl PlateCache {
    fn get(&self, source: &[u8], mask: &[u8]) -> Option<&RgbImage> {
        let hash = key_hash(source, mask);
        self.entries
            .iter()
            .find(|e| e.hash == hash && e.source == source && e.mask == mask)
            .map(|e| &e.filled)
    }

    fn insert(&mut self, source: Vec<u8>, mask: Vec<u8>, filled: RgbImage) {
        let entry = CachedPlate {
            hash: key_hash(&source, &mask),
            source,
            mask,
            filled,
        };
        let size = entry.bytes();
        if size > CACHE_BYTES {
            return;
        }
        while self.entries.len() >= CACHE_ENTRIES || self.bytes + size > CACHE_BYTES {
            let Some(oldest) = self.entries.pop_front() else {
                break;
            };
            self.bytes -= oldest.bytes();
        }
        self.bytes += size;
        self.entries.push_back(entry);
    }
}

fn key_hash(source: &[u8], mask: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    source.hash(&mut hasher);
    mask.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "tests/inpaint.rs"]
mod tests;

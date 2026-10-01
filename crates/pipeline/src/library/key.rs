//! The key of a sign in the library: its Japanese, normalised, and the difference hash of its
//! keyframe crop.
//!
//! **Role:** turn an occurrence's reading into the text the library compares (Unicode NFKC, every
//! whitespace character removed), hash its crop to 64 bits that change little when the picture
//! changes a little, and say how far apart two hashes are.
//!
//! **Position:** used by `library` for every key it reads or writes and by `library::signs` for
//! the occurrences of a job.
//!
//! **Signals and state:** reads one PNG per hash; no other state.
//!
//! **Invariants:** a sign matches only the same normalised Japanese at a Hamming distance of at
//! most `MATCH_DISTANCE`; the hash depends only on the decoded pixels.

use std::path::Path;

use image::imageops::FilterType;
use unicode_normalization::UnicodeNormalization;

use crate::error::{Context, Result};

/// The most bits two crop hashes of one sign may differ in.
pub const MATCH_DISTANCE: u32 = 6;

/// The largest crop the hash decodes, in either direction.
const MAX_CROP_EDGE: u32 = 8192;

/// `japanese` in Unicode NFKC with every whitespace character removed.
pub fn normalised(japanese: &str) -> String {
    japanese.nfkc().filter(|c| !c.is_whitespace()).collect()
}

/// The 64-bit difference hash of `picture`: shrunk to 9 by 8 grey pixels, one bit per pair of
/// horizontal neighbours, set where the left one is brighter, rows top to bottom.
pub fn difference_hash(picture: &image::DynamicImage) -> u64 {
    let small = picture
        .grayscale()
        .resize_exact(9, 8, FilterType::Triangle)
        .to_luma8();
    let mut hash = 0u64;
    for y in 0..8 {
        for x in 0..8 {
            let brighter = small.get_pixel(x, y)[0] > small.get_pixel(x + 1, y)[0];
            hash = (hash << 1) | u64::from(brighter);
        }
    }
    hash
}

/// The difference hash of the PNG crop at `path`.
pub fn crop_hash(path: &Path) -> Result<u64> {
    let at = format!("crop {}", path.display());
    let mut reader = image::ImageReader::open(path).context(&at)?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_CROP_EDGE);
    limits.max_image_height = Some(MAX_CROP_EDGE);
    reader.limits(limits);
    let picture = reader
        .with_guessed_format()
        .context(&at)?
        .decode()
        .context(&at)?;
    Ok(difference_hash(&picture))
}

/// How many bits `a` and `b` differ in.
pub fn distance(a: u64, b: u64) -> u32 {
    (a ^ b).count_ones()
}

#[cfg(test)]
#[path = "tests/key.rs"]
mod tests;

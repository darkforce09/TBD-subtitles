//! What the scan reads from the brightness plane alone: a thumbnail of block means for the
//! duplicate check, and grey crops for region signatures, with no colour conversion.
//!
//! **Role:** summarise a picture's luma in small cells, compare two summaries block by block, and
//! copy a rectangle of luma as full-range grey.
//!
//! **Position:** used by the detection scan; the thumbnail replaces an RGB comparison of whole
//! frames, the grey crop an RGB crop followed by a grey conversion.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** cells at the right and bottom edges average only the pixels they cover; two
//! thumbnails compare only when their grids agree; a grey crop's rectangle lies inside the
//! picture or nothing is written.

use super::colour::Coefficients;
use super::convert::{Rect, Yuv420};

/// A picture's luma averaged over `cell` × `cell` squares, row by row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LumaThumbnail {
    pub cell: usize,
    pub columns: usize,
    pub rows: usize,
    pub means: Vec<u8>,
}

/// The mean luma of every `cell` × `cell` square of `picture`.
pub fn luma_thumbnail(picture: &Yuv420<'_>, cell: usize) -> LumaThumbnail {
    let cell = cell.max(1);
    let columns = picture.width.div_ceil(cell);
    let rows = picture.height.div_ceil(cell);
    let mut sums = vec![0u32; columns * rows];
    let mut counts = vec![0u32; columns * rows];
    for row in 0..picture.height {
        let base = (row / cell) * columns;
        for (column, &sample) in picture.luma_row(row).iter().enumerate() {
            sums[base + column / cell] += u32::from(sample);
            counts[base + column / cell] += 1;
        }
    }
    let means = sums
        .iter()
        .zip(&counts)
        .map(|(sum, count)| ((sum + count / 2) / count.max(&1)) as u8)
        .collect();
    LumaThumbnail {
        cell,
        columns,
        rows,
        means,
    }
}

impl LumaThumbnail {
    /// Whether every block of `group` × `group` cells differs from `other`'s by at most
    /// `max_mean_difference` levels on average; `false` when the grids differ.
    pub fn matches(&self, other: &LumaThumbnail, group: usize, max_mean_difference: u32) -> bool {
        if (self.cell, self.columns, self.rows) != (other.cell, other.columns, other.rows) {
            return false;
        }
        let group = group.max(1);
        for block_row in (0..self.rows).step_by(group) {
            for block_column in (0..self.columns).step_by(group) {
                let mut total = 0u32;
                let mut count = 0u32;
                for row in block_row..(block_row + group).min(self.rows) {
                    for column in block_column..(block_column + group).min(self.columns) {
                        let at = row * self.columns + column;
                        total += u32::from(self.means[at].abs_diff(other.means[at]));
                        count += 1;
                    }
                }
                if total > max_mean_difference * count {
                    return false;
                }
            }
        }
        true
    }
}

/// The luma of `rect` of `picture` as full-range grey into `grey`, resized to the rectangle;
/// `false`, writing nothing, when the rectangle is not inside the picture.
pub fn crop_grey(
    picture: &Yuv420<'_>,
    colour: &Coefficients,
    rect: Rect,
    grey: &mut Vec<u8>,
) -> bool {
    if !rect.inside(picture) {
        return false;
    }
    grey.clear();
    for row in rect.y..rect.y + rect.height {
        let samples = &picture.luma_row(row)[rect.x..rect.x + rect.width];
        grey.extend(samples.iter().map(|&y| colour.grey(y)));
    }
    true
}

#[cfg(test)]
#[path = "tests/luma.rs"]
mod tests;

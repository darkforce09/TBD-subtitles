//! Line breaking and size fitting of English lettering inside a rectified writing area.
//!
//! **Role:** choose the lines, the size and the width axis of one occurrence's lettering.
//! **Position:** pure layout math under `compose`; the font supplies metrics through `Metrics`.
//! **Signals and state:** English text, the rectified area and a target cap height in; a
//! `Layout` with line positions out.
//! **Invariants:** a layout never exceeds 92 % of the area's width or 90 % of its height; the
//! wider axis is always tried before a smaller size; sizes never exceed the target.

use job_model::onscreen::LetteringStyle;

/// Width-axis values tried at each size, widest first.
pub(crate) const WIDTH_AXES: [f64; 3] = [100.0, 87.5, 75.0];
/// Baseline-to-baseline distance in ems.
pub(crate) const LINE_SPACING: f64 = 1.15;
/// Share of the area's width the longest line may use.
const WIDTH_SHARE: f64 = 0.92;
/// Share of the area's height the block of lines may use.
const HEIGHT_SHARE: f64 = 0.90;
/// Factor by which the size shrinks between attempts.
const SHRINK: f64 = 0.96;
/// Target cap height as a share of the original line height.
pub(crate) const CAP_SHARE: f64 = 0.7;

/// Font measurements the layout needs, for one weight.
pub(crate) trait Metrics {
    /// Horizontal advance of `line` in pixels at `size` pixels per em and `width_axis`.
    fn line_width(&self, line: &str, size: f64, width_axis: f64) -> f64;
    /// Cap height in ems.
    fn cap_ratio(&self) -> f64;
    /// Descender depth in ems, positive.
    fn descent_ratio(&self) -> f64;
}

/// The rectified writing area in source pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    pub width: f64,
    pub height: f64,
}

impl Area {
    /// Tall, narrow areas hold vertical Japanese; English stacks one word per line there.
    pub(crate) fn vertical(self) -> bool {
        self.height > 1.6 * self.width
    }
}

/// Lettering that fits its area.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Layout {
    pub lines: Vec<String>,
    /// Pixels per em.
    pub size: f64,
    /// Cap height in pixels.
    pub cap: f64,
    pub width_axis: f64,
}

/// One line's pen origin in area pixels: left edge and baseline.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Placement {
    pub text: String,
    pub x: f64,
    pub baseline: f64,
}

/// The `wght` axis value that matches the original stroke weight.
pub(crate) fn weight(style: &LetteringStyle) -> f64 {
    let ratio = if style.line_height_px > 0.0 {
        style.stroke_px / style.line_height_px
    } else {
        0.0
    };
    if ratio < 0.08 {
        400.0
    } else if ratio < 0.12 {
        600.0
    } else if ratio < 0.16 {
        700.0
    } else {
        900.0
    }
}

/// The cap height that keeps the original writing's size.
pub(crate) fn target_cap(style: &LetteringStyle) -> f64 {
    CAP_SHARE * style.line_height_px
}

/// The largest layout with a cap height between `min_cap` and `target`; `None` when even
/// `min_cap` does not fit.
pub(crate) fn fit(
    metrics: &dyn Metrics,
    english: &str,
    area: Area,
    target: f64,
    min_cap: f64,
) -> Option<Layout> {
    if !(target.is_finite() && target > 0.0) || target < min_cap {
        return None;
    }
    let mut cap = target;
    loop {
        if let Some(layout) = fit_at(metrics, english, area, cap) {
            return Some(layout);
        }
        let next = cap * SHRINK;
        if next >= min_cap && next > 0.01 {
            cap = next;
        } else if cap > min_cap && min_cap > 0.0 {
            cap = min_cap;
        } else {
            return None;
        }
    }
}

/// A layout at exactly `cap`, trying the width axes widest first.
pub(crate) fn fit_at(metrics: &dyn Metrics, english: &str, area: Area, cap: f64) -> Option<Layout> {
    let cap_ratio = metrics.cap_ratio();
    if !(cap_ratio > 0.0 && cap.is_finite() && cap > 0.0) {
        return None;
    }
    let size = cap / cap_ratio;
    let max_width = WIDTH_SHARE * area.width;
    WIDTH_AXES.iter().find_map(|&width_axis| {
        let lines = wrap(
            metrics,
            english,
            max_width,
            size,
            width_axis,
            area.vertical(),
        )?;
        (block_height(metrics, lines.len(), size) <= HEIGHT_SHARE * area.height).then_some(Layout {
            lines,
            size,
            cap,
            width_axis,
        })
    })
}

/// Height of `count` lines from the first cap top to the last descender.
pub(crate) fn block_height(metrics: &dyn Metrics, count: usize, size: f64) -> f64 {
    if count == 0 {
        return 0.0;
    }
    (count - 1) as f64 * LINE_SPACING * size
        + (metrics.cap_ratio() + metrics.descent_ratio()) * size
}

/// Break `english` at spaces so every line fits `max_width`; explicit line breaks are kept and
/// `stacked` puts every word on its own line. `None` when one word alone is too wide.
pub(crate) fn wrap(
    metrics: &dyn Metrics,
    english: &str,
    max_width: f64,
    size: f64,
    width_axis: f64,
    stacked: bool,
) -> Option<Vec<String>> {
    let mut lines = Vec::new();
    for paragraph in english.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if metrics.line_width(word, size, width_axis) > max_width {
                return None;
            }
            if line.is_empty() {
                line.push_str(word);
                continue;
            }
            let joined = format!("{line} {word}");
            if !stacked && metrics.line_width(&joined, size, width_axis) <= max_width {
                line = joined;
            } else {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
    }
    (!lines.is_empty()).then_some(lines)
}

/// Pen origins of each line, centred horizontally and the block centred vertically.
pub(crate) fn placements(metrics: &dyn Metrics, layout: &Layout, area: Area) -> Vec<Placement> {
    let block = block_height(metrics, layout.lines.len(), layout.size);
    let top = (area.height - block) / 2.0;
    layout
        .lines
        .iter()
        .enumerate()
        .map(|(index, line)| Placement {
            text: line.clone(),
            x: (area.width - metrics.line_width(line, layout.size, layout.width_axis)) / 2.0,
            baseline: top + layout.cap + index as f64 * LINE_SPACING * layout.size,
        })
        .collect()
}

/// The common factor for a container's members: the largest `k ≤ 1` at which every member's
/// target cap height, scaled by `k`, still fits. Each pair is (target, best fitting cap).
pub(crate) fn shared_scale(members: &[(f64, f64)]) -> f64 {
    members
        .iter()
        .filter(|(target, _)| *target > 0.0)
        .map(|(target, best)| best / target)
        .fold(1.0f64, f64::min)
}

#[cfg(test)]
#[path = "tests/layout.rs"]
mod tests;

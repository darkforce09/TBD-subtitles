//! English visible-writing events on the shared ASS canvas.
//!
//! **Role:** render safe surface replacements or readable nearby labels alongside dialogue.
//! **Position:** final visual stage before the ordinary subtitle output writer.
//! **Signals and state:** one font and frame-timed ASS events; stable placement per occurrence.
//! **Invariants:** unsafe surfaces are never covered; the lower dialogue band stays clear;
//! overlapping fallback labels are packed; adjacent identical visuals coalesce.

use job_model::onscreen::{Point, Quad, TextDocument, TextOccurrence, TextTreatment};

use super::{
    TextResult,
    event_buffer::{Event, EventBuffer},
    geometry,
    glyphs::{self, Font},
};

const WIDTH: f64 = 1920.0;
const TEXT_BOTTOM: f64 = 820.0;
type Rect = (f64, f64, f64, f64);
type LabelLayout = (Rect, f64, String);

#[derive(Clone, Copy)]
struct Occupied {
    start: f64,
    end: f64,
    rect: (f64, f64, f64, f64),
}

pub fn events(document: &mut TextDocument) -> TextResult<String> {
    if document.occurrences.is_empty() {
        return Ok(String::new());
    }
    if document.width == 0 || document.height == 0 {
        return Err("Text typesetting needs source video dimensions".into());
    }
    let sx = WIDTH / f64::from(document.width);
    let sy = 1080.0 / f64::from(document.height);
    let font = Font::load().ok();
    let mut output = EventBuffer::default();
    let mut occupied = Vec::new();
    let mut nearby = Vec::new();
    for (index, occurrence) in document.occurrences.iter_mut().enumerate() {
        occurrence.rendered = Some(false);
        occurrence
            .warnings
            .retain(|warning| !warning.starts_with("No rendered translation:"));
        if !occurrence.reviewed
            && (!occurrence.confidence.is_finite() || occurrence.confidence < 0.85)
        {
            warn(
                occurrence,
                "No rendered translation: The reading or translation is uncertain; review its candidate English before displaying it.",
            );
            continue;
        }
        let Some(english) = occurrence
            .english
            .as_ref()
            .filter(|s| !s.trim().is_empty())
            .cloned()
        else {
            warn(
                occurrence,
                "No reliable English reading is available; no translation is fabricated.",
            );
            continue;
        };
        if !occurrence.start_s.is_finite()
            || !occurrence.end_s.is_finite()
            || occurrence.start_s < 0.0
            || occurrence.end_s <= occurrence.start_s
        {
            return Err("Text typesetting received invalid occurrence timing".into());
        }
        let safe = safe_surface(occurrence, sx, sy);
        if occurrence.presentation.treatment != TextTreatment::Nearby && safe {
            if let Some(font) = &font {
                match replacement(occurrence, &english, font, sx, sy) {
                    Ok(events) => {
                        output.append(events)?;
                        occupied.push(Occupied {
                            start: occurrence.start_s,
                            end: occurrence.end_s,
                            rect: union_bounds(occurrence, sx, sy),
                        });
                        occurrence.presentation.treatment = TextTreatment::Replace;
                        occurrence.rendered = Some(true);
                        continue;
                    }
                    Err(error) => warn(
                        occurrence,
                        &format!("Sign lettering uses nearby placement: {error}"),
                    ),
                }
            } else {
                warn(
                    occurrence,
                    "No outline font is available; English uses nearby placement.",
                );
            }
        } else if occurrence.presentation.treatment != TextTreatment::Nearby {
            warn(
                occurrence,
                "The surface or motion is uncertain; English uses nearby placement.",
            );
        }
        occurrence.presentation.treatment = TextTreatment::Nearby;
        nearby.push((index, english));
    }
    nearby.sort_by(|a, b| {
        document.occurrences[a.0]
            .start_s
            .total_cmp(&document.occurrences[b.0].start_s)
    });
    for (index, english) in nearby {
        let occurrence = &mut document.occurrences[index];
        let preferred = occurrence.presentation.anchor.map(|p| Point {
            x: p.x * sx,
            y: p.y * sy,
        });
        let initial_size = occurrence
            .presentation
            .font_size
            .map(|s| s * sy)
            .unwrap_or(36.0)
            .clamp(14.0, 400.0);
        let placement = place_label(
            occurrence,
            &english,
            preferred,
            initial_size,
            &occupied,
            font.as_ref(),
        );
        let (rect, size, wrapped) = match placement {
            Ok(placement) => placement,
            Err(error) => {
                warn(
                    occurrence,
                    &format!("No rendered translation: {error}. English is retained for review."),
                );
                continue;
            }
        };
        if size + 0.1 < initial_size {
            warn(
                occurrence,
                "English text size is reduced to avoid overlapping simultaneous translations.",
            );
        }
        occupied.push(Occupied {
            start: occurrence.start_s,
            end: occurrence.end_s,
            rect,
        });
        let (left, top, right, bottom) = rect;
        occurrence.rendered = Some(true);
        let background = format!(
            "{{\\an7\\pos(0,0)\\p1\\bord0\\shad0\\1c&H000000&\\1a&H60&}}m {left:.0} {top:.0} l {right:.0} {top:.0} {right:.0} {bottom:.0} {left:.0} {bottom:.0}"
        );
        let font_tag = font
            .as_ref()
            .map(|font| format!("\\fn{}", safe_font_name(&font.name)))
            .unwrap_or_default();
        let text = format!(
            "{{\\an7\\pos({:.2},{:.2})\\fs{size:.2}{font_tag}\\fscx100\\fscy100\\bord2\\shad0\\1c&HFFFFFF&\\3c&H000000&\\q2}}{}",
            left + 10.0,
            top + 8.0,
            escape(&wrapped)
        );
        output.push(event(20, occurrence.start_s, occurrence.end_s, background))?;
        output.push(event(21, occurrence.start_s, occurrence.end_s, text))?;
    }
    output.events.sort_by(|a, b| {
        a.start
            .cmp(&b.start)
            .then(a.layer.cmp(&b.layer))
            .then(a.end.cmp(&b.end))
    });
    Ok(output
        .events
        .into_iter()
        .map(|event| {
            format!(
                "Dialogue: {},{},{},Default,On-screen Text,0,0,0,,{}\n",
                event.layer,
                timestamp(event.start),
                timestamp(event.end),
                event.text
            )
        })
        .collect())
}

fn safe_surface(text: &TextOccurrence, sx: f64, sy: f64) -> bool {
    !text.frames.is_empty()
        && text
            .frames
            .first()
            .is_some_and(|f| text.start_s >= f.time_s - 1e-5)
        && text
            .frames
            .last()
            .is_some_and(|f| text.end_s <= f.end_s + 1e-5)
        && !text.warnings.iter().any(|warning| {
            let warning = warning.to_ascii_lowercase();
            ["tracking", "occlusion", "unsafe", "surface", "uncertain"]
                .iter()
                .any(|word| warning.contains(word))
        })
        && text.frames.iter().all(|frame| {
            let quad = scaled(frame.quad, sx, sy);
            let (l, t, r, b) = quad.bounds();
            frame.surface_rgb.is_some()
                && frame.time_s.is_finite()
                && frame.end_s.is_finite()
                && frame.end_s > frame.time_s
                && geometry::quad_to_quad(quad, quad).is_some()
                && l >= 0.0
                && t >= 0.0
                && r <= WIDTH
                && b <= TEXT_BOTTOM
        })
}

fn replacement(
    text: &TextOccurrence,
    english: &str,
    font: &Font,
    sx: f64,
    sy: f64,
) -> TextResult<EventBuffer> {
    if let Some(character) = font.missing_glyph(english) {
        return Err(format!("The sign font has no glyph for {character:?}").into());
    }
    let mut events = EventBuffer::default();
    // One occurrence keeps a stable background palette instead of amplifying sampling noise.
    let rgb = text
        .frames
        .first()
        .and_then(|frame| frame.surface_rgb)
        .ok_or("Missing surface color")?;
    for frame in &text.frames {
        let start = frame.time_s.max(text.start_s);
        let end = frame.end_s.min(text.end_s);
        if end <= start {
            continue;
        }
        let quad = scaled(frame.quad, sx, sy);
        let color = format!("{:02X}{:02X}{:02X}", rgb[2], rgb[1], rgb[0]);
        let foreground = if u32::from(rgb[0]) * 299
            + u32::from(rgb[1]) * 587
            + u32::from(rgb[2]) * 114
            > 140_000
        {
            "101010"
        } else {
            "FFFFFF"
        };
        let polygon = polygon(quad);
        let mask = format!("{{\\an7\\pos(0,0)\\p4\\bord0\\shad0\\1c&H{color}&\\1a&H00&}}{polygon}");
        let quad = glyphs::anchored_quad(
            quad,
            text.presentation.anchor.map(|p| Point {
                x: p.x * sx,
                y: p.y * sy,
            }),
        )?;
        let (left, top, right, bottom) = quad.bounds();
        let height = bottom - top;
        let requested = text.presentation.font_size.map(|s| s * sy);
        let lettering = if flat(quad) {
            let size = requested.unwrap_or(height * 0.70).min(height * 0.70);
            let wrapped = wrap(english, size, (right - left) * 0.86, Some(font));
            let lines = wrapped.lines().count().max(1) as f64;
            let size = size.min(height * 0.80 / (lines * 1.3));
            if size < 14.0 {
                return Err("English lettering is too small to read on this surface".into());
            }
            let measured = wrapped
                .lines()
                .map(|line| font.width(line, size))
                .fold(0.0f64, f64::max);
            let horizontal = (100.0 * (right - left) * 0.86 / measured.max(1.0)).min(100.0);
            format!(
                "{{\\an5\\pos({:.2},{:.2})\\fs{size:.2}\\fn{}\\fscx{horizontal:.2}\\fscy100\\bord0\\shad0\\1c&H{foreground}&\\q2}}{}",
                (left + right) / 2.0,
                (top + bottom) / 2.0,
                safe_font_name(&font.name),
                escape(&wrapped)
            )
        } else {
            let drawing = font.drawing(english, quad, requested)?;
            format!("{{\\an7\\pos(0,0)\\p4\\bord0\\shad0\\1c&H{foreground}&}}{drawing}")
        };
        events.push(event(10, start, end, mask))?;
        events.push(event(11, start, end, lettering))?;
    }
    if events.events.is_empty() {
        return Err("No detected frames intersect the corrected text timing".into());
    }
    Ok(events)
}

fn place_label(
    text: &TextOccurrence,
    english: &str,
    preferred: Option<Point>,
    initial_size: f64,
    occupied: &[Occupied],
    font: Option<&Font>,
) -> TextResult<LabelLayout> {
    let mut size = initial_size;
    while size >= 14.0 {
        let wrapped = wrap(english, size, 900.0, font);
        let width = wrapped
            .lines()
            .map(|line| measure(line, size, font))
            .fold(0.0f64, f64::max)
            .min(1840.0)
            + 20.0;
        let height = wrapped.lines().count().max(1) as f64 * size * 1.4 + 16.0;
        let mut candidates = Vec::new();
        if let Some(anchor) = preferred {
            candidates.push((
                (anchor.x - width / 2.0).clamp(24.0, (1896.0 - width).max(24.0)),
                anchor.y.max(24.0),
            ));
        }
        let mut y = 24.0;
        while y + height <= TEXT_BOTTOM {
            for x in [24.0, (WIDTH - width) / 2.0, WIDTH - 24.0 - width] {
                candidates.push((x, y));
            }
            y += height + 12.0;
        }
        for (x, y) in candidates {
            let rect = (x, y, x + width, y + height);
            if rect.2 > WIDTH - 24.0 || rect.3 > TEXT_BOTTOM {
                continue;
            }
            if !occupied.iter().any(|other| {
                other.start < text.end_s && text.start_s < other.end && overlaps(rect, other.rect)
            }) {
                return Ok((rect, size, wrapped));
            }
        }
        size -= 2.0;
    }
    Err(format!("Cannot place all simultaneous translations without overlap near {} seconds; reduce text size or adjust timing", text.start_s).into())
}

fn wrap(text: &str, size: f64, width: f64, font: Option<&Font>) -> String {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if measure(word, size, font) > width {
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                for character in word.chars() {
                    let candidate = format!("{line}{character}");
                    if !line.is_empty() && measure(&candidate, size, font) > width {
                        lines.push(std::mem::take(&mut line));
                    }
                    line.push(character);
                }
                continue;
            }
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && measure(&candidate, size, font) > width {
                lines.push(line);
                line = word.to_owned();
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    lines.join("\n")
}

fn measure(text: &str, size: f64, font: Option<&Font>) -> f64 {
    font.map(|font| font.width(text, size))
        .unwrap_or(text.chars().count() as f64 * size * 0.62)
}
fn overlaps(a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)) -> bool {
    a.0 < b.2 + 10.0 && b.0 < a.2 + 10.0 && a.1 < b.3 + 10.0 && b.1 < a.3 + 10.0
}
fn scaled(quad: Quad, sx: f64, sy: f64) -> Quad {
    Quad(quad.0.map(|p| Point {
        x: p.x * sx,
        y: p.y * sy,
    }))
}
fn flat(q: Quad) -> bool {
    (q.0[0].y - q.0[1].y).abs() < 0.5
        && (q.0[2].y - q.0[3].y).abs() < 0.5
        && (q.0[0].x - q.0[3].x).abs() < 0.5
        && (q.0[1].x - q.0[2].x).abs() < 0.5
}
fn union_bounds(text: &TextOccurrence, sx: f64, sy: f64) -> (f64, f64, f64, f64) {
    text.frames
        .iter()
        .map(|f| scaled(f.quad, sx, sy).bounds())
        .fold(
            (
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ),
            |(l, t, r, b), (a, c, d, e)| (l.min(a), t.min(c), r.max(d), b.max(e)),
        )
}
fn polygon(q: Quad) -> String {
    let p = q.0.map(|p| {
        format!(
            "{} {}",
            (p.x * 8.0).round() as i64,
            (p.y * 8.0).round() as i64
        )
    });
    format!("m {} l {} {} {} {}", p[0], p[1], p[2], p[3], p[0])
}
fn event(layer: u8, start: f64, end: f64, text: String) -> Event {
    let start = (start * 100.0).round().max(0.0) as u64;
    let end = (end * 100.0).round().max(0.0) as u64;
    Event {
        layer,
        start,
        end: end.max(start + 1),
        text,
    }
}
fn timestamp(cs: u64) -> String {
    format!(
        "{}:{:02}:{:02}.{:02}",
        cs / 360000,
        cs / 6000 % 60,
        cs / 100 % 60,
        cs % 100
    )
}
fn warn(text: &mut TextOccurrence, message: &str) {
    if !text.warnings.iter().any(|w| w == message) {
        text.warnings.push(message.to_owned());
    }
}
fn safe_font_name(name: &str) -> String {
    name.chars()
        .filter(|c| !matches!(c, '{' | '}' | '\\' | '\r' | '\n'))
        .collect()
}
fn escape(text: &str) -> String {
    text.chars()
        .filter(|c| *c != '\r')
        .map(|c| match c {
            '\n' => "\\N".into(),
            '{' => "\\{".into(),
            '}' => "\\}".into(),
            '\\' => "\\\u{2060}".into(),
            c if c.is_control() => " ".into(),
            c => c.to_string(),
        })
        .collect()
}

#[cfg(test)]
#[path = "tests/typeset.rs"]
mod tests;

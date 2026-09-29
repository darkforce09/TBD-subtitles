//! Fail-closed checks against owner-reviewed visual annotations.
//!
//! **Role:** measure coverage, timing and tracking separately from model confidence.
//! **Position:** validation harness only; never changes production artifacts.
//! **Signals and state:** annotation JSON and a machine-readable verdict, including invalid inputs.
//! **Invariants:** each readable annotation needs its own actual occurrence; unresolved geometry
//! needs an explicit nearby fallback, and malformed input never leaves a stale passing verdict.

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use job_model::onscreen::{Point, Quad, TextDocument, TextOccurrence, TextTreatment};
use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
struct Annotations {
    fps: f64,
    height: u32,
    occurrences: Vec<Expected>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Expected {
    label: String,
    japanese: String,
    start_s: f64,
    end_s: f64,
    #[serde(default)]
    frames: Vec<Frame>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Frame {
    time_s: f64,
    quad: Quad,
}

#[derive(Default, Deserialize, Serialize)]
struct Verdict {
    passed: bool,
    readable: usize,
    translated: usize,
    flagged: usize,
    failures: Vec<String>,
}

pub(super) fn run(actual: &Path, annotations: &Path, output: &Path) -> Result<()> {
    let evaluated = (|| {
        let document: TextDocument = serde_json::from_slice(&std::fs::read(actual)?)
            .context("invalid actual visual document")?;
        let expected: Annotations =
            serde_json::from_slice(&std::fs::read(annotations)?).context("invalid annotations")?;
        evaluate(&document, &expected)
    })();
    let verdict = evaluated.unwrap_or_else(|error: anyhow::Error| Verdict {
        failures: vec![format!("{error:#}")],
        ..Verdict::default()
    });
    pipeline::work_dir::write_json(output, &verdict)?;
    ensure!(
        verdict.passed,
        "visual validation failed; see {}",
        output.display()
    );
    Ok(())
}

fn evaluate(document: &TextDocument, expected: &Annotations) -> Result<Verdict> {
    validate(document, expected)?;
    let tolerance = 1.0 / expected.fps + 1e-6;
    let edges: Vec<Vec<usize>> = expected
        .occurrences
        .iter()
        .map(|entry| {
            document
                .occurrences
                .iter()
                .enumerate()
                .filter(|(_, item)| identity_matches(item, entry))
                .filter(|(_, item)| check(item, entry, tolerance, expected.height).is_ok())
                .map(|(index, _)| index)
                .collect()
        })
        .collect();
    let mut owners = vec![None; document.occurrences.len()];
    for index in 0..expected.occurrences.len() {
        assign(
            index,
            &edges,
            &mut owners,
            &mut vec![false; document.occurrences.len()],
        );
    }
    let mut assignments = vec![None; expected.occurrences.len()];
    for (index, owner) in owners.into_iter().enumerate() {
        if let Some(owner) = owner {
            assignments[owner] = Some(index);
        }
    }
    let mut verdict = Verdict {
        readable: expected.occurrences.len(),
        ..Verdict::default()
    };
    for (index, entry) in expected.occurrences.iter().enumerate() {
        if let Some(actual) = assignments[index] {
            if flagged(&document.occurrences[actual]) {
                verdict.flagged += 1;
            } else {
                verdict.translated += 1;
            }
            continue;
        }
        let detail = if !edges[index].is_empty() {
            "no distinct actual occurrence; another annotation already uses this track".to_string()
        } else {
            document
                .occurrences
                .iter()
                .filter(|item| identity_matches(item, entry))
                .find_map(|item| check(item, entry, tolerance, expected.height).err())
                .unwrap_or_else(|| {
                    "readable occurrence missing or source reading mismatched".into()
                })
        };
        verdict.failures.push(format!("{}: {detail}", entry.label));
    }
    verdict.passed = verdict.failures.is_empty();
    Ok(verdict)
}

/// An augmenting match avoids both track reuse and order-dependent greedy failures.
fn assign(
    entry: usize,
    edges: &[Vec<usize>],
    owners: &mut [Option<usize>],
    seen: &mut [bool],
) -> bool {
    for &actual in &edges[entry] {
        if seen[actual] {
            continue;
        }
        seen[actual] = true;
        if owners[actual].is_none_or(|previous| assign(previous, edges, owners, seen)) {
            owners[actual] = Some(entry);
            return true;
        }
    }
    false
}

fn validate(document: &TextDocument, expected: &Annotations) -> Result<()> {
    ensure!(
        expected.fps.is_finite()
            && expected.fps > 0.0
            && expected.height > 0
            && !expected.occurrences.is_empty(),
        "annotations need a positive rate, height and at least one readable occurrence"
    );
    ensure!(
        document.width > 0 && document.height == expected.height,
        "actual picture dimensions must match the annotated source height"
    );
    let mut labels = BTreeSet::new();
    for entry in &expected.occurrences {
        ensure!(
            !entry.label.trim().is_empty() && labels.insert(entry.label.trim()),
            "annotation labels must be nonempty and unique"
        );
        ensure!(
            !normalize(&entry.japanese).is_empty(),
            "{}: annotation needs readable Japanese",
            entry.label
        );
        ensure!(
            valid_times(entry.start_s, entry.end_s),
            "{}: invalid annotation timing",
            entry.label
        );
        ensure!(
            !entry.frames.is_empty(),
            "{}: annotation needs frame geometry",
            entry.label
        );
        let mut previous = None;
        for frame in &entry.frames {
            ensure!(
                frame.time_s.is_finite()
                    && frame.time_s >= entry.start_s
                    && frame.time_s < entry.end_s
                    && previous.is_none_or(|time| frame.time_s > time),
                "{}: annotation frame times must increase inside its interval",
                entry.label
            );
            ensure!(
                valid_quad(frame.quad, document.width, document.height),
                "{}: invalid annotation geometry",
                entry.label
            );
            previous = Some(frame.time_s);
        }
    }
    let mut ids = BTreeSet::new();
    for item in &document.occurrences {
        ensure!(
            !item.id.trim().is_empty() && ids.insert(&item.id),
            "actual occurrence IDs must be nonempty and unique"
        );
        ensure!(
            valid_times(item.start_s, item.end_s),
            "{}: invalid actual timing",
            item.id
        );
        ensure!(
            !item.frames.is_empty(),
            "{}: actual track has no frame geometry",
            item.id
        );
        let mut previous = None;
        for frame in &item.frames {
            ensure!(
                valid_times(frame.time_s, frame.end_s)
                    && previous.is_none_or(|time| frame.time_s > time)
                    && valid_quad(frame.quad, document.width, document.height),
                "{}: invalid actual frame timing or geometry",
                item.id
            );
            previous = Some(frame.time_s);
        }
    }
    Ok(())
}

fn check(
    item: &TextOccurrence,
    expected: &Expected,
    tolerance: f64,
    height: u32,
) -> Result<(), String> {
    if (item.start_s - expected.start_s).abs() > tolerance
        || (item.end_s - expected.end_s).abs() > tolerance
    {
        return Err("timing exceeds one source frame".into());
    }
    let unreadable = normalize(&item.japanese).is_empty();
    let is_flagged = flagged(item);
    let translated = item
        .english
        .as_ref()
        .is_some_and(|text| !text.trim().is_empty())
        && item.rendered == Some(true);
    if !is_flagged && !translated {
        return Err("neither rendered English nor an explicit unresolved warning".into());
    }
    if unreadable
        && (!is_flagged
            || item
                .english
                .as_ref()
                .is_some_and(|text| !text.trim().is_empty()))
    {
        return Err("unreadable writing must be flagged without invented English".into());
    }
    let fallback = is_flagged && item.presentation.treatment == TextTreatment::Nearby;
    for sample in &expected.frames {
        // Sparse observations hold their geometry until the next one; a sample inside that
        // interval, or within one source frame of its start, is covered by it.
        let frame = item
            .frames
            .iter()
            .find(|frame| {
                (frame.time_s - sample.time_s).abs() <= tolerance
                    || (frame.time_s <= sample.time_s && sample.time_s < frame.end_s)
            })
            .ok_or_else(|| {
                format!(
                    "track has no observation covering {:.3}s within one source frame",
                    sample.time_s
                )
            })?;
        if unreadable || fallback {
            if !overlaps(frame.quad, sample.quad) {
                return Err(format!(
                    "flagged track does not overlap annotated text at {:.3}s",
                    sample.time_s
                ));
            }
        } else {
            let error = frame
                .quad
                .0
                .iter()
                .zip(sample.quad.0)
                .map(|(a, b)| (a.x - b.x).hypot(a.y - b.y))
                .fold(0.0, f64::max)
                * 1080.0
                / f64::from(height);
            if error > 2.0 + 1e-6 {
                return Err(format!(
                    "tracking error {error:.2}px at {:.3}s without flagged fallback",
                    sample.time_s
                ));
            }
        }
    }
    Ok(())
}

fn flagged(item: &TextOccurrence) -> bool {
    !item.reviewed
        && item
            .warnings
            .iter()
            .any(|warning| !warning.trim().is_empty())
}

fn identity_matches(item: &TextOccurrence, expected: &Expected) -> bool {
    let japanese = normalize(&item.japanese);
    japanese == normalize(&expected.japanese) || (japanese.is_empty() && flagged(item))
}

fn valid_times(start_s: f64, end_s: f64) -> bool {
    start_s.is_finite() && end_s.is_finite() && start_s >= 0.0 && end_s > start_s
}

fn normalize(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn valid_quad(quad: Quad, width: u32, height: u32) -> bool {
    if !quad.valid()
        || quad
            .0
            .iter()
            .any(|p| p.x < 0.0 || p.y < 0.0 || p.x > f64::from(width) || p.y > f64::from(height))
    {
        return false;
    }
    // Clockwise in image coordinates, with no folded or degenerate corner.
    (0..4).all(|index| {
        let a = quad.0[index];
        let b = quad.0[(index + 1) % 4];
        let c = quad.0[(index + 2) % 4];
        (b.x - a.x) * (c.y - b.y) - (b.y - a.y) * (c.x - b.x) > 1e-9
    })
}

fn overlaps(a: Quad, b: Quad) -> bool {
    let mut polygon = a.0.to_vec();
    for index in 0..4 {
        let start = b.0[index];
        let end = b.0[(index + 1) % 4];
        let input = std::mem::take(&mut polygon);
        if input.is_empty() {
            return false;
        }
        let mut previous = *input.last().expect("nonempty clipped polygon");
        let mut before = side(start, end, previous);
        for current in input {
            let after = side(start, end, current);
            if (before >= 0.0) != (after >= 0.0) {
                let ratio = before / (before - after);
                polygon.push(Point {
                    x: previous.x + ratio * (current.x - previous.x),
                    y: previous.y + ratio * (current.y - previous.y),
                });
            }
            if after >= 0.0 {
                polygon.push(current);
            }
            previous = current;
            before = after;
        }
    }
    area(&polygon) > 0.5 * area(&a.0).max(area(&b.0))
}

fn side(start: Point, end: Point, point: Point) -> f64 {
    (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x)
}

fn area(points: &[Point]) -> f64 {
    points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .map(|(a, b)| a.x * b.y - b.x * a.y)
        .sum::<f64>()
        .abs()
        / 2.0
}

#[cfg(test)]
#[path = "tests/evaluate.rs"]
mod tests;

//! Conservative reuse of scene-matched reference subtitle wording.
//!
//! **Role:** find a reference sign only when dialogue anchors match the dub and its wording
//! agrees with an independent translation of the visible reading.
//! **Position:** called during visual translation, never used to transfer placement or timing.
//! **Signals and state:** parsed ASS text, bounded to 32 MiB per reference file.
//! **Invariants:** missing references do not stop translation; mismatched edits supply no wording.

use super::TextResult;
use std::path::{Path, PathBuf};
use subtitle_formats::cue::{CueKind, CueTrack};

#[derive(Debug, Clone)]
struct Line {
    start: f64,
    end: f64,
    text: String,
}

pub struct Reference {
    path: PathBuf,
    lines: Vec<Line>,
}

pub fn load(folder: Option<&Path>, excluded: Option<&Path>) -> TextResult<Vec<Reference>> {
    let Some(folder) = folder else {
        return Ok(Vec::new());
    };
    if !folder.is_dir() {
        tracing::warn!(path = %folder.display(), "Reference folder is unavailable; using local translation");
        return Ok(Vec::new());
    }
    let mut files = std::fs::read_dir(folder)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ass")))
        .collect::<Vec<_>>();
    files.sort();
    let mut references = Vec::new();
    for path in files {
        if excluded.is_some_and(|own| {
            path == own
                || path
                    .canonicalize()
                    .ok()
                    .zip(own.canonicalize().ok())
                    .is_some_and(|(a, b)| a == b)
        }) {
            continue;
        }
        if std::fs::metadata(&path)?.len() > 32 * 1024 * 1024 {
            return Err("reference ASS exceeds 32 MiB".into());
        }
        let text = std::fs::read_to_string(&path)?;
        let lines = text.lines().filter_map(parse_line).collect();
        references.push(Reference { path, lines });
    }
    Ok(references)
}

pub fn verified(
    references: &[Reference],
    dialogue: &CueTrack,
    at: f64,
    translated: &str,
) -> Option<(String, PathBuf)> {
    let translation = normal(translated);
    if !at.is_finite() || at < 0.0 || translation.is_empty() {
        return None;
    }
    let context: Vec<(f64, String)> = dialogue
        .cues
        .iter()
        .filter(|cue| cue.kind == CueKind::Dialogue)
        .filter_map(|cue| {
            let time = dialogue.frame_rate.millis(cue.start) as f64 / 1000.0;
            let text = normal(
                &cue.lines
                    .iter()
                    .map(|l| l.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" "),
            );
            ((time - at).abs() < 25.0 && text.len() >= 24).then_some((time, text))
        })
        .collect();
    for reference in references {
        for (dub_time, spoken) in &context {
            for anchor in reference
                .lines
                .iter()
                .filter(|l| normal(&l.text) == *spoken)
            {
                let mapped = at + anchor.start - dub_time;
                if let Some(sign) = reference.lines.iter().find(|line| {
                    line.start <= mapped + 1.0
                        && line.end >= mapped - 1.0
                        && normal(&line.text) == translation
                }) {
                    return Some((sign.text.clone(), reference.path.clone()));
                }
            }
        }
    }
    None
}

fn normal(text: &str) -> String {
    text.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
fn parse_line(text: &str) -> Option<Line> {
    let fields = text
        .strip_prefix("Dialogue:")?
        .trim()
        .splitn(10, ',')
        .collect::<Vec<_>>();
    if fields.len() != 10 {
        return None;
    }
    let start = time(fields[1])?;
    let end = time(fields[2])?;
    if end <= start {
        return None;
    }
    let mut plain = String::new();
    let mut tag = None;
    for c in fields[9].chars() {
        match c {
            '{' if tag.is_none() => tag = Some(String::new()),
            '}' => {
                if drawing(&tag.take()?) {
                    return None;
                }
            }
            '{' => return None,
            _ => match &mut tag {
                Some(tag) => tag.push(c),
                None => plain.push(c),
            },
        }
    }
    if tag.is_some() {
        return None;
    }
    let text = plain
        .replace("\\N", " ")
        .replace("\\n", " ")
        .replace("\\h", " ");
    (!normal(&text).is_empty()).then_some(Line { start, end, text })
}

fn drawing(tags: &str) -> bool {
    tags.split('\\').skip(1).any(|tag| {
        tag.strip_prefix('p').is_some_and(|value| {
            value
                .trim_start()
                .chars()
                .take_while(char::is_ascii_digit)
                .any(|digit| digit != '0')
        })
    })
}

fn time(text: &str) -> Option<f64> {
    let mut parts = text.trim().split(':');
    let hours = parts.next()?;
    let minutes = parts.next()?;
    let seconds = parts.next()?;
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    if parts.next().is_some() || !digits(hours) || !digits(minutes) {
        return None;
    }
    let mut decimal = seconds.split('.');
    if !digits(decimal.next()?)
        || decimal.next().is_some_and(|fraction| !digits(fraction))
        || decimal.next().is_some()
    {
        return None;
    }
    let hours = hours.parse::<u64>().ok()? as f64;
    let minutes = minutes.parse::<u8>().ok()?;
    let seconds = seconds.parse::<f64>().ok()?;
    (minutes < 60 && (0.0..60.0).contains(&seconds))
        .then_some(hours * 3600.0 + f64::from(minutes) * 60.0 + seconds)
        .filter(|time| time.is_finite())
}

#[cfg(test)]
#[path = "tests/reference.rs"]
mod tests;

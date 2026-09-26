//! Sound-cue candidates: the detector's events, Whisper's sound tags and the songs, cut down to
//! what could matter before the language model chooses.
//!
//! **Role:** turn raw events into a short, time-ordered list the model can read: songs from the
//! runs of lyric lines; effects outside songs; non-speech voices only where nobody is talking and
//! with stricter limits for the classes that over-fire (groans, sighs); Whisper's `*tags*`; each
//! class merged across short gaps.
//!
//! **Position:** called by the sound-cue step before the language model; reads the sheet, the
//! first-pass lines and Whisper's transcript.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** music under dialogue is never a candidate (only songs are); a candidate keeps
//! the times its source gave; ids run `S001`, `S002`, … in time order.

use std::collections::HashMap;

use job_model::outputs::{
    CandidateKind, EngineTranscript, Line, SoundCandidate, SoundEvent, TimeSpan, Utterance,
};

/// Same-class events closer than this merge into one candidate.
pub const MERGE_GAP_S: f64 = 1.0;
/// Lyric lines closer than this, with no spoken line between, belong to one song.
pub const SONG_GAP_S: f64 = 30.0;
/// A voice event overlapping speech for more than this share of its length is taken for speech.
pub const SPEECH_OVERLAP: f64 = 0.5;
/// Stricter `(class, minimum peak, minimum seconds)` for voice classes that fire too often.
pub const STRICT_VOICES: &[(&str, f32, f64)] =
    &[("Groan", 0.6, 1.0), ("Sigh", 0.6, 1.0), ("Grunt", 0.5, 0.5)];
/// Classes that describe music, which reaches the cues only as songs.
pub const MUSIC_CLASSES: &[&str] = &["Music", "Singing"];

/// The candidates, in time order with ids, and the song spans.
pub fn candidates(
    events: &[SoundEvent],
    sheet: &[Utterance],
    lines: &[Line],
    whisper: &EngineTranscript,
) -> (Vec<SoundCandidate>, Vec<TimeSpan>) {
    let songs = song_spans(sheet, lines);
    let speech = speech_words(sheet, lines);
    let in_song = |start: f64, end: f64| songs.iter().any(|s| start < s.end_s && end > s.start_s);
    let mut found: Vec<SoundCandidate> = Vec::new();
    for span in &songs {
        found.push(candidate(
            CandidateKind::Song,
            "Song",
            span.start_s,
            span.end_s,
            1.0,
        ));
    }
    for event in events {
        if MUSIC_CLASSES.contains(&event.label.as_str()) || in_song(event.start_s, event.end_s) {
            continue;
        }
        if event.stem == "vocals" {
            if let Some(&(_, peak, min_s)) =
                STRICT_VOICES.iter().find(|(c, _, _)| *c == event.label)
                && (event.peak < peak || event.end_s - event.start_s < min_s)
            {
                continue;
            }
            if speech_share(&speech, event.start_s, event.end_s) > SPEECH_OVERLAP {
                continue;
            }
            found.push(candidate(
                CandidateKind::Voice,
                &event.label,
                event.start_s,
                event.end_s,
                event.peak,
            ));
        } else {
            found.push(candidate(
                CandidateKind::Effect,
                &event.label,
                event.start_s,
                event.end_s,
                event.peak,
            ));
        }
    }
    for word in whisper.words() {
        if let Some(tag) = sound_tag(&word.text)
            && !in_song(word.start_s, word.end_s)
        {
            found.push(candidate(
                CandidateKind::Tag,
                &tag,
                word.start_s,
                word.end_s,
                1.0,
            ));
        }
    }
    found.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    let mut merged: Vec<SoundCandidate> = Vec::new();
    for c in found {
        let joined =
            merged.iter_mut().rev().take(8).find(|m| {
                m.kind == c.kind && m.label == c.label && c.start_s - m.end_s < MERGE_GAP_S
            });
        match joined {
            Some(m) if c.kind != CandidateKind::Song => {
                m.end_s = m.end_s.max(c.end_s);
                m.peak = m.peak.max(c.peak);
            }
            _ => merged.push(c),
        }
    }
    for (i, c) in merged.iter_mut().enumerate() {
        c.id = format!("S{:03}", i + 1);
    }
    (merged, songs)
}

/// The spans of runs of lyric lines. Two lyric lines belong to one song when no spoken line lies
/// between them and they are less than `SONG_GAP_S` apart (an instrumental break).
pub fn song_spans(sheet: &[Utterance], lines: &[Line]) -> Vec<TimeSpan> {
    let mut spans: Vec<TimeSpan> = Vec::new();
    let by_id = lines_by_id(lines);
    let mut spoken_since = true;
    for u in sheet {
        let Some(line) = by_id.get(u.id.as_str()) else {
            continue;
        };
        if !line.has_flag("LYRIC") {
            spoken_since |= !line.has_flag("DROP");
            continue;
        }
        match spans.last_mut() {
            Some(last) if !spoken_since && u.start_s - last.end_s < SONG_GAP_S => {
                last.end_s = last.end_s.max(u.end_s)
            }
            _ => spans.push(TimeSpan::new(u.start_s, u.end_s)),
        }
        spoken_since = false;
    }
    spans
}

/// A Whisper sound tag such as `*Grunting*`, `[laughs]` or `(sighs)`, as lowercase words.
pub fn sound_tag(word: &str) -> Option<String> {
    let trimmed = word.trim_end_matches(['.', ',', '!', '?']);
    let inner = [('*', '*'), ('[', ']'), ('(', ')')]
        .iter()
        .find_map(|&(open, close)| {
            trimmed
                .strip_prefix(open)
                .and_then(|t| t.strip_suffix(close))
        })?;
    let inner = inner.trim();
    (!inner.is_empty() && inner.chars().any(char::is_alphabetic)).then(|| inner.to_lowercase())
}

/// The backbone's word times of every kept (not lyric, not dropped) utterance.
fn speech_words(sheet: &[Utterance], lines: &[Line]) -> Vec<(f64, f64)> {
    let by_id = lines_by_id(lines);
    sheet
        .iter()
        .filter(|u| {
            by_id
                .get(u.id.as_str())
                .is_some_and(|l| !l.has_flag("LYRIC") && !l.has_flag("DROP"))
        })
        .flat_map(|u| u.words.iter().map(|w| (w.start_s, w.end_s)))
        .collect()
}

/// The share of `[start, end)` covered by speech words.
fn speech_share(words: &[(f64, f64)], start: f64, end: f64) -> f64 {
    let length = end - start;
    if length <= 0.0 {
        return 1.0;
    }
    let covered: f64 = words
        .iter()
        .map(|&(a, b)| (b.min(end) - a.max(start)).max(0.0))
        .sum();
    (covered / length).min(1.0)
}

fn lines_by_id(lines: &[Line]) -> HashMap<&str, &Line> {
    lines.iter().map(|l| (l.id.as_str(), l)).collect()
}

fn candidate(
    kind: CandidateKind,
    label: &str,
    start_s: f64,
    end_s: f64,
    peak: f32,
) -> SoundCandidate {
    SoundCandidate {
        id: String::new(),
        kind,
        label: label.to_string(),
        start_s,
        end_s,
        peak,
    }
}

#[cfg(test)]
#[path = "tests/candidates.rs"]
mod tests;

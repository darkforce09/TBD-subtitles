//! The diff sheet: the backbone engine's words cut into utterances, with every other engine's
//! disagreements written inline, as the language model reads it.
//!
//! **Role:** align each other engine's words to the backbone's, chunk by chunk; cut the backbone
//! into utterances at pauses and sentence ends; and render one line per utterance, such as
//! `U0412 12:03.4 2.1s | Law, the {P:Birdcage|W:bird cage} is closing{W:+in}!`.
//!
//! **Position:** called by the diff-sheet stage and the stack spike tool; uses `align.rs`.
//!
//! **Signals and state:** none; pure over the transcripts.
//!
//! **Invariants:** every backbone word lands in exactly one utterance; a word every engine agrees
//! on is locked; each utterance keeps every engine's own words for it, for the novelty check.

use job_model::outputs::{EngineTranscript, TimedWord};

use super::align::{self, Step};

/// A pause this long ends an utterance.
pub const PAUSE_S: f64 = 0.6;
/// A sentence end followed by a pause this long ends an utterance.
pub const SENTENCE_PAUSE_S: f64 = 0.2;
/// No utterance runs longer.
pub const MAX_UTTERANCE_S: f64 = 12.0;

pub use job_model::outputs::Utterance;

/// What another engine heard against one backbone word.
#[derive(Debug, Clone, Default)]
struct Against {
    /// The word it heard in the backbone word's place; `None` when it heard nothing there.
    replaced: Option<String>,
    /// Words it heard after this word that the backbone lacks.
    inserted: Vec<String>,
}

/// Build the sheet. `backbone` and each of `others` must share the chunk plan; `tags` gives one
/// letter per engine, backbone first (`P`, `W`, …).
pub fn build(
    backbone: &EngineTranscript,
    others: &[&EngineTranscript],
    tags: &[&str],
) -> Vec<Utterance> {
    let mut utterances = Vec::new();
    for (c, chunk) in backbone.chunks.iter().enumerate() {
        let norm_backbone = normalised(&chunk.words);
        // per engine, per backbone word
        let mut against: Vec<Vec<Against>> = Vec::new();
        let mut leading: Vec<Vec<String>> = Vec::new();
        for other in others {
            let words = other
                .chunks
                .get(c)
                .map(|ch| ch.words.as_slice())
                .unwrap_or(&[]);
            let (per_word, before) = compare(&chunk.words, &norm_backbone, words);
            against.push(per_word);
            leading.push(before);
        }
        let mut start = 0;
        for end in cut_points(&chunk.words) {
            utterances.push(render(
                utterances.len(),
                &chunk.words[start..end],
                &norm_backbone[start..end],
                &against.iter().map(|a| &a[start..end]).collect::<Vec<_>>(),
                if start == 0 { Some(&leading) } else { None },
                tags,
            ));
            start = end;
        }
    }
    utterances
}

fn normalised(words: &[TimedWord]) -> Vec<String> {
    words.iter().map(|w| align::normalise(&w.text)).collect()
}

/// Align `other` to the backbone words; returns what it heard per backbone word, and the words it
/// heard before the first backbone word.
fn compare(
    backbone: &[TimedWord],
    norm: &[String],
    other: &[TimedWord],
) -> (Vec<Against>, Vec<String>) {
    // Compare only words that fold to something; keep indices back to the originals.
    let a_idx: Vec<usize> = (0..backbone.len())
        .filter(|&i| !norm[i].is_empty())
        .collect();
    let b_norm = normalised(other);
    let b_idx: Vec<usize> = (0..other.len())
        .filter(|&j| !b_norm[j].is_empty())
        .collect();
    let a: Vec<String> = a_idx.iter().map(|&i| norm[i].clone()).collect();
    let b: Vec<String> = b_idx.iter().map(|&j| b_norm[j].clone()).collect();
    let mut per_word = vec![Against::default(); backbone.len()];
    // Words the backbone folds to nothing count as agreed.
    for (i, n) in norm.iter().enumerate() {
        if n.is_empty() {
            per_word[i].replaced = Some(backbone[i].text.clone());
        }
    }
    let mut before = Vec::new();
    let mut last: Option<usize> = None;
    for step in align::align(&a, &b) {
        match step {
            Step::Match(i, j) | Step::Substitute(i, j) => {
                per_word[a_idx[i]].replaced = Some(other[b_idx[j]].text.clone());
                last = Some(a_idx[i]);
            }
            Step::Delete(i) => last = Some(a_idx[i]),
            Step::Insert(j) => match last {
                Some(i) => per_word[i].inserted.push(other[b_idx[j]].text.clone()),
                None => before.push(other[b_idx[j]].text.clone()),
            },
        }
    }
    (per_word, before)
}

/// Where utterances end inside one chunk: exclusive word indices, the last one the word count.
fn cut_points(words: &[TimedWord]) -> Vec<usize> {
    let mut cuts = Vec::new();
    let mut start_s = words.first().map(|w| w.start_s).unwrap_or(0.0);
    for i in 0..words.len() {
        let Some(next) = words.get(i + 1) else {
            break;
        };
        let gap = next.start_s - words[i].end_s;
        let sentence_end = words[i].text.ends_with(['.', '?', '!']);
        let too_long = next.end_s - start_s > MAX_UTTERANCE_S;
        if gap >= PAUSE_S || (sentence_end && gap >= SENTENCE_PAUSE_S) || too_long {
            cuts.push(i + 1);
            start_s = next.start_s;
        }
    }
    if !words.is_empty() {
        cuts.push(words.len());
    }
    cuts
}

fn render(
    number: usize,
    words: &[TimedWord],
    norm: &[String],
    against: &[&[Against]],
    leading: Option<&Vec<Vec<String>>>,
    tags: &[&str],
) -> Utterance {
    let (start_s, end_s) = (words[0].start_s, words[words.len() - 1].end_s);
    let id = format!("U{:04}", number + 1);
    let mut line = format!("{id} {} {:.1}s |", clock(start_s), end_s - start_s);
    let mut hypotheses: Vec<(String, Vec<String>)> = vec![(
        tags.first().copied().unwrap_or("P").to_string(),
        words.iter().map(|w| w.text.clone()).collect(),
    )];
    for e in 0..against.len() {
        let mut heard: Vec<String> = leading.map(|l| l[e].clone()).unwrap_or_default();
        for a in against[e] {
            heard.extend(a.replaced.clone());
            heard.extend(a.inserted.iter().cloned());
        }
        hypotheses.push((tags.get(e + 1).copied().unwrap_or("X").to_string(), heard));
    }
    if let Some(leading) = leading {
        for (e, before) in leading.iter().enumerate() {
            if !before.is_empty() {
                line.push_str(&format!(
                    " {{{}:+{}}}",
                    tags.get(e + 1).copied().unwrap_or("X"),
                    before.join(" ")
                ));
            }
        }
    }
    let mut locked = Vec::with_capacity(words.len());
    for (i, word) in words.iter().enumerate() {
        let differing: Vec<(usize, String)> = (0..against.len())
            .filter_map(|e| {
                let heard = against[e][i].replaced.clone();
                let same = heard.as_deref().map(align::normalise) == Some(norm[i].clone());
                (!same).then(|| (e, heard.unwrap_or_else(|| "∅".to_string())))
            })
            .collect();
        locked.push(differing.is_empty());
        line.push(' ');
        if differing.is_empty() {
            line.push_str(&word.text);
        } else {
            let mut options = vec![format!(
                "{}:{}",
                tags.first().copied().unwrap_or("P"),
                word.text
            )];
            for (e, heard) in differing {
                options.push(format!(
                    "{}:{heard}",
                    tags.get(e + 1).copied().unwrap_or("X")
                ));
            }
            line.push_str(&format!("{{{}}}", options.join("|")));
        }
        for (e, a) in against.iter().enumerate() {
            if !a[i].inserted.is_empty() {
                line.push_str(&format!(
                    "{{{}:+{}}}",
                    tags.get(e + 1).copied().unwrap_or("X"),
                    a[i].inserted.join(" ")
                ));
            }
        }
    }
    Utterance {
        id,
        start_s,
        end_s,
        words: words.to_vec(),
        locked,
        line,
        hypotheses,
    }
}

/// `m:ss.d`, as the sheet shows times.
fn clock(seconds: f64) -> String {
    let tenths = (seconds * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, (tenths / 10) % 60, tenths % 10)
}

#[cfg(test)]
#[path = "tests/sheet.rs"]
mod tests;

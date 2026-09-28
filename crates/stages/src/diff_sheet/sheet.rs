//! The diff sheet: the backbone engine's words cut into utterances, with every other engine's
//! disagreements written inline, as the language model reads it.
//!
//! **Role:** align each other engine's words to the backbone's, chunk by chunk; cut the backbone
//! into utterances at pauses and sentence ends; render one line per utterance, such as
//! `U0412 12:03.4 2.1s | Law, the {P:Birdcage|W:bird cage} is closing{W:+in}!`; and give each
//! utterance the span in which any engine heard its words.
//!
//! **Position:** called by the diff-sheet stage and the stack spike tool, and for the heard spans
//! by the alignment and review tasks; uses `align.rs`.
//!
//! **Signals and state:** none; pure over the transcripts.
//!
//! **Invariants:** every backbone word lands in exactly one utterance; a word every engine agrees
//! on is locked; each utterance keeps every engine's own words for it, for the novelty check;
//! `build` and `heard_spans` cut the same utterances, in the same order.

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
    /// The indices of its words behind `replaced` and `inserted`, into its chunk's words.
    heard: Vec<usize>,
}

/// One utterance's share of its chunk: its backbone words and what each other engine heard
/// against them.
struct Piece<'a> {
    /// The backbone words.
    words: &'a [TimedWord],
    /// The backbone words folded for comparison.
    norm: &'a [String],
    /// Per other engine: its words of the whole chunk.
    other_words: &'a [&'a [TimedWord]],
    /// Per other engine: what it heard against each backbone word.
    against: Vec<&'a [Against]>,
    /// Per other engine: the indices of its words heard before the chunk's first backbone word;
    /// empty except in the chunk's first utterance.
    leading: Vec<&'a [usize]>,
}

/// Build the sheet. `backbone` and each of `others` must share the chunk plan; `tags` gives one
/// letter per engine, backbone first (`P`, `W`, …).
pub fn build(
    backbone: &EngineTranscript,
    others: &[&EngineTranscript],
    tags: &[&str],
) -> Vec<Utterance> {
    let mut utterances = Vec::new();
    for_each_piece(backbone, others, |piece| {
        let number = utterances.len();
        utterances.push(render(number, &piece, tags));
    });
    utterances
}

/// Per utterance of `build(backbone, others, …)`, in its order: the earliest start and latest end
/// of its backbone words and of every other engine's word it holds (heard before it, in a
/// backbone word's place, or between its words).
pub fn heard_spans(backbone: &EngineTranscript, others: &[&EngineTranscript]) -> Vec<(f64, f64)> {
    let mut spans = Vec::new();
    for_each_piece(backbone, others, |piece| spans.push(piece.heard_span()));
    spans
}

/// Compare every other engine to the backbone chunk by chunk, cut each chunk into utterances,
/// and hand each utterance's piece to `each`, in order.
fn for_each_piece(
    backbone: &EngineTranscript,
    others: &[&EngineTranscript],
    mut each: impl FnMut(Piece<'_>),
) {
    for (c, chunk) in backbone.chunks.iter().enumerate() {
        let norm = normalised(&chunk.words);
        let other_words: Vec<&[TimedWord]> = others
            .iter()
            .map(|other| {
                other
                    .chunks
                    .get(c)
                    .map(|ch| ch.words.as_slice())
                    .unwrap_or(&[])
            })
            .collect();
        let compared: Vec<(Vec<Against>, Vec<usize>)> = other_words
            .iter()
            .map(|words| compare(&chunk.words, &norm, words))
            .collect();
        let mut start = 0;
        for end in cut_points(&chunk.words) {
            each(Piece {
                words: &chunk.words[start..end],
                norm: &norm[start..end],
                other_words: &other_words,
                against: compared.iter().map(|(a, _)| &a[start..end]).collect(),
                leading: compared
                    .iter()
                    .map(|(_, before)| if start == 0 { before.as_slice() } else { &[] })
                    .collect(),
            });
            start = end;
        }
    }
}

impl Piece<'_> {
    /// The texts engine `e` heard before the chunk's first backbone word.
    fn leading_texts(&self, e: usize) -> Vec<String> {
        self.leading[e]
            .iter()
            .map(|&j| self.other_words[e][j].text.clone())
            .collect()
    }

    fn heard_span(&self) -> (f64, f64) {
        let mut span = (f64::INFINITY, f64::NEG_INFINITY);
        let mut widen = |w: &TimedWord| {
            span = (span.0.min(w.start_s), span.1.max(w.end_s));
        };
        self.words.iter().for_each(&mut widen);
        for (e, words) in self.other_words.iter().enumerate() {
            let held = self.against[e].iter().flat_map(|a| &a.heard);
            for &j in self.leading[e].iter().chain(held) {
                widen(&words[j]);
            }
        }
        span
    }
}

fn normalised(words: &[TimedWord]) -> Vec<String> {
    words.iter().map(|w| align::normalise(&w.text)).collect()
}

/// Align `other` to the backbone words; returns what it heard per backbone word, and the indices
/// of the words it heard before the first backbone word.
fn compare(
    backbone: &[TimedWord],
    norm: &[String],
    other: &[TimedWord],
) -> (Vec<Against>, Vec<usize>) {
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
                let against = &mut per_word[a_idx[i]];
                against.replaced = Some(other[b_idx[j]].text.clone());
                against.heard.push(b_idx[j]);
                last = Some(a_idx[i]);
            }
            Step::Delete(i) => last = Some(a_idx[i]),
            Step::Insert(j) => match last {
                Some(i) => {
                    per_word[i].inserted.push(other[b_idx[j]].text.clone());
                    per_word[i].heard.push(b_idx[j]);
                }
                None => before.push(b_idx[j]),
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

fn render(number: usize, piece: &Piece<'_>, tags: &[&str]) -> Utterance {
    let Piece {
        words,
        norm,
        against,
        ..
    } = piece;
    let (start_s, end_s) = (words[0].start_s, words[words.len() - 1].end_s);
    let id = format!("U{:04}", number + 1);
    let mut line = format!("{id} {} {:.1}s |", clock(start_s), end_s - start_s);
    let mut hypotheses: Vec<(String, Vec<String>)> = vec![(
        tags.first().copied().unwrap_or("P").to_string(),
        words.iter().map(|w| w.text.clone()).collect(),
    )];
    for (e, engine) in against.iter().enumerate() {
        let mut heard = piece.leading_texts(e);
        for a in engine.iter() {
            heard.extend(a.replaced.clone());
            heard.extend(a.inserted.iter().cloned());
        }
        hypotheses.push((tags.get(e + 1).copied().unwrap_or("X").to_string(), heard));
    }
    for e in 0..against.len() {
        let before = piece.leading_texts(e);
        if !before.is_empty() {
            line.push_str(&format!(
                " {{{}:+{}}}",
                tags.get(e + 1).copied().unwrap_or("X"),
                before.join(" ")
            ));
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
/// A time as the sheet writes it: `m:ss.d`.
pub fn clock(seconds: f64) -> String {
    let tenths = (seconds * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, (tenths / 10) % 60, tenths % 10)
}

#[cfg(test)]
#[path = "tests/sheet.rs"]
mod tests;

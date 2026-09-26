//! Line breaking: one line when the text fits in 42 characters, else the two-line break that
//! reads best by the Netflix rules.
//!
//! **Role:** score every place a two-line cue could break: after punctuation and before a
//! conjunction or preposition is good; after an article, a determiner, a possessive, a
//! preposition or a subject pronoun, or between two capitalised words (a name), is bad; a shorter
//! top line is preferred and one or two words alone on top are avoided.
//!
//! **Position:** used by `segment.rs` (does a unit fit?) and by `mod.rs` (the final lines).
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** no returned line is longer than `MAX_LINE`; never more than two lines; words
//! keep their order and spelling.

/// Characters per line.
pub const MAX_LINE: usize = 42;

const ARTICLES: &[&str] = &[
    "a", "an", "the", "this", "that", "these", "those", "my", "your", "his", "her", "its", "our",
    "their", "some", "any", "no", "every", "each",
];
const SUBJECTS: &[&str] = &[
    "i", "you", "he", "she", "we", "they", "i'm", "you're", "he's", "she's", "we're", "they're",
];
const PREPOSITIONS: &[&str] = &[
    "of", "to", "in", "on", "at", "by", "for", "with", "from", "into", "onto", "about", "over",
    "under", "after", "before", "through", "without", "against", "between",
];
const CONJUNCTIONS: &[&str] = &[
    "and", "but", "or", "so", "because", "if", "when", "while", "since", "until", "unless",
    "although", "though", "that", "which", "who", "where",
];

/// The text as one or two lines, or `None` when it cannot fit in two lines of `MAX_LINE`.
pub fn layout(words: &[&str]) -> Option<Vec<String>> {
    let one = words.join(" ");
    if one.chars().count() <= MAX_LINE {
        return Some(vec![one]);
    }
    let lengths: Vec<usize> = words.iter().map(|w| w.chars().count()).collect();
    let mut best: Option<(f64, usize)> = None;
    for k in 1..words.len() {
        let top = lengths[..k].iter().sum::<usize>() + k - 1;
        let bottom = lengths[k..].iter().sum::<usize>() + words.len() - k - 1;
        if top > MAX_LINE || bottom > MAX_LINE {
            continue;
        }
        let score = break_score(words, k, top, bottom);
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, k));
        }
    }
    best.map(|(_, k)| vec![words[..k].join(" "), words[k..].join(" ")])
}

/// Whether the words fit in at most two lines.
pub fn fits(words: &[&str]) -> bool {
    layout(words).is_some()
}

/// Lower is better.
fn break_score(words: &[&str], k: usize, top: usize, bottom: usize) -> f64 {
    let last = words[k - 1];
    let next = words[k];
    let last_bare = bare(last);
    let next_bare = bare(next);
    let mut score = 0.0;
    // Shape: bottom-heavy, and not too uneven.
    score += top.saturating_sub(bottom) as f64 * 1.5;
    score += top.abs_diff(bottom) as f64 * 0.3;
    if k <= 2 && words.len() > 4 {
        score += 60.0;
    }
    // Good places.
    if last.ends_with(['.', '!', '?', '…']) {
        score -= 40.0;
    } else if last.ends_with([',', ';', ':', '—']) || last.ends_with("--") {
        score -= 30.0;
    } else if CONJUNCTIONS.contains(&next_bare.as_str())
        || PREPOSITIONS.contains(&next_bare.as_str())
    {
        score -= 15.0;
    }
    // Bad places.
    if ARTICLES.contains(&last_bare.as_str()) {
        score += 60.0;
    }
    if PREPOSITIONS.contains(&last_bare.as_str()) || CONJUNCTIONS.contains(&last_bare.as_str()) {
        score += 35.0;
    }
    if SUBJECTS.contains(&last_bare.as_str()) {
        score += 40.0;
    }
    let capital = |w: &str| w.chars().next().is_some_and(char::is_uppercase);
    let sentence_start = k < 2 || words[k - 2].ends_with(['.', '!', '?', '…']);
    if capital(last)
        && capital(next)
        && !last.ends_with(|c: char| !c.is_alphanumeric())
        && !sentence_start
    {
        score += 45.0;
    }
    score
}

/// Whether a unit should not end on `word`: an article, a preposition, a conjunction or a subject
/// pronoun, which all belong with the word after.
pub fn weak_end(word: &str) -> bool {
    let bare = bare(word);
    [ARTICLES, PREPOSITIONS, CONJUNCTIONS, SUBJECTS]
        .iter()
        .any(|list| list.contains(&bare.as_str()))
}

/// Lowercase, without surrounding punctuation.
fn bare(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_lowercase()
}

#[cfg(test)]
#[path = "tests/line_break.rs"]
mod tests;

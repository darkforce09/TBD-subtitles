//! Whether what the OCR read over a finished replacement shows it lettered cleanly.
//!
//! **Role:** judge one sampled frame from its readings (Japanese still there, English read back
//! too differently, or twice) and an occurrence from all of its frames.
//! **Position:** the string half of the read-back check; `verify` feeds it the readings of the
//! lines found over each sample's lettering.
//! **Signals and state:** pure functions of strings and confidences.
//! **Invariants:** a frame passes only when no reading holds Japanese and the letters and digits
//! read are close enough to the English; an occurrence passes only when every frame does, and a
//! frame with Japanese decides the reason over one that reads back badly.

use job_model::onscreen::VerifyReading;

/// Why a replacement whose finished picture still shows Japanese stays in the subtitle file.
pub const JAPANESE_LEFT: &str = "The finished picture still shows Japanese";
/// Why a replacement whose English does not read back stays in the subtitle file.
pub const UNREADABLE: &str = "The English does not read back cleanly";
/// Kana or kanji one reading needs to count as Japanese.
pub const MIN_JAPANESE_CHARS: usize = 2;
/// Confidence a reading needs to count as Japanese.
pub const MIN_JAPANESE_CONFIDENCE: f64 = 0.5;
/// The least similarity between the English and its reading.
pub const MIN_SIMILARITY: f64 = 0.6;

/// One line the OCR read around the lettering, in reading order.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadLine {
    pub text: String,
    pub confidence: f64,
    /// It lies over the lettering: its reading is part of the English read back.
    pub over_lettering: bool,
    /// It lies where the original writing was: Japanese in it was not erased.
    pub over_writing: bool,
}

/// Judge one frame: Japanese read where the writing was, and the English read over the lettering
/// against `english`.
pub fn judge(frame: u64, lines: &[ReadLine], english: &str) -> VerifyReading {
    let japanese: Vec<&str> = lines
        .iter()
        .filter(|line| {
            line.over_writing
                && line.confidence >= MIN_JAPANESE_CONFIDENCE
                && japanese_chars(&line.text) >= MIN_JAPANESE_CHARS
        })
        .map(|line| line.text.as_str())
        .collect();
    let read = lines
        .iter()
        .filter(|line| line.over_lettering && !letters(&line.text).is_empty())
        .map(|line| line.text.trim())
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let similarity = similarity(&read, english);
    VerifyReading {
        frame,
        japanese_found: japanese.join(" "),
        english_read: read,
        similarity,
        passed: japanese.is_empty() && similarity >= MIN_SIMILARITY,
    }
}

/// Why an occurrence with `readings` stays in the subtitle file, or none when every frame passed.
pub fn verdict(readings: &[VerifyReading]) -> Option<&'static str> {
    if readings
        .iter()
        .any(|reading| !reading.japanese_found.is_empty())
    {
        Some(JAPANESE_LEFT)
    } else if readings.iter().any(|reading| !reading.passed) {
        Some(UNREADABLE)
    } else {
        None
    }
}

/// Hiragana, katakana and CJK unified ideographs in `text`.
pub fn japanese_chars(text: &str) -> usize {
    text.chars()
        .filter(|&c| {
            matches!(c,
                '\u{3041}'..='\u{309F}'
                | '\u{30A0}'..='\u{30FF}'
                | '\u{31F0}'..='\u{31FF}'
                | '\u{3400}'..='\u{4DBF}'
                | '\u{4E00}'..='\u{9FFF}'
                | '\u{FF66}'..='\u{FF9D}')
                && c != '\u{30FB}'
                && c != '\u{30FC}'
        })
        .count()
}

/// The letters and digits of `text`, lowercased, with Latin accents and fullwidth forms folded
/// to ASCII; everything else is dropped.
pub fn letters(text: &str) -> String {
    text.chars()
        .filter_map(|c| {
            let c = fold(c).to_ascii_lowercase();
            c.is_ascii_alphanumeric().then_some(c)
        })
        .collect()
}

/// How closely the English appears in what was read, over letters and digits: 1 − the edit
/// distance of the English to its best-matching stretch of the reading ÷ the English's length.
/// Letters read outside that stretch cost nothing, since the reading area can reach a
/// neighbouring line's English; an English found a second time in them means the lettering is
/// doubled and scores 0. 1 when neither has letters, 0 when only the reading has.
pub fn similarity(read: &str, english: &str) -> f64 {
    let (read, english) = (letters(read), letters(english));
    let (read, english) = (read.as_bytes(), english.as_bytes());
    if english.is_empty() {
        return if read.is_empty() { 1.0 } else { 0.0 };
    }
    let score = |distance: usize| (1.0 - distance as f64 / english.len() as f64).max(0.0);
    let (distance, start, end) = best_stretch(read, english);
    let found_again = |rest: &[u8]| score(best_stretch(rest, english).0) >= MIN_SIMILARITY;
    if found_again(&read[..start]) || found_again(&read[end..]) {
        return 0.0;
    }
    score(distance)
}

/// The fewest single-byte edits turning some stretch `read[start..end]` into `english`, with
/// that stretch; reading bytes outside it are free.
fn best_stretch(read: &[u8], english: &[u8]) -> (usize, usize, usize) {
    // Row `i` holds, for each reading prefix, the cost of matching `english[..i]` ending there
    // and where that match started.
    let mut previous: Vec<(usize, usize)> = (0..=read.len()).map(|j| (0, j)).collect();
    for (i, &y) in english.iter().enumerate() {
        let mut current = vec![(i + 1, 0); read.len() + 1];
        for (j, &x) in read.iter().enumerate() {
            let substitute = (previous[j].0 + usize::from(x != y), previous[j].1);
            let skip_english = (previous[j + 1].0 + 1, previous[j + 1].1);
            let skip_read = (current[j].0 + 1, current[j].1);
            current[j + 1] = [substitute, skip_english, skip_read]
                .into_iter()
                .min_by_key(|&(cost, _)| cost)
                .unwrap_or(substitute);
        }
        previous = current;
    }
    let (end, &(distance, start)) = previous
        .iter()
        .enumerate()
        .min_by_key(|&(end, &(cost, _))| (cost, end))
        .unwrap_or((0, &(english.len(), 0)));
    (distance, start, end)
}

/// The ASCII letter or digit a fullwidth or accented Latin character stands for, else itself.
fn fold(c: char) -> char {
    let code = u32::from(c);
    if (0xFF10..=0xFF19).contains(&code)
        || (0xFF21..=0xFF3A).contains(&code)
        || (0xFF41..=0xFF5A).contains(&code)
    {
        return char::from_u32(code - 0xFEE0).unwrap_or(c);
    }
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' => {
            'a'
        }
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'È' | 'É' | 'Ê' | 'Ë' | 'Ē' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ō' | 'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ō' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' => 'u',
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        _ => c,
    }
}

#[cfg(test)]
#[path = "tests/verdict.rs"]
mod tests;

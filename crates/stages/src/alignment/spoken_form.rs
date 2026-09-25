//! Displayed words to the words a speaker says, for an aligner that knows only spoken letters.
//!
//! **Role:** fold a displayed word to lowercase spoken words: accents dropped, digits read out,
//! hyphens and slashes split, symbols spelled, punctuation removed, and keep which displayed word
//! each spoken word came from.
//!
//! **Position:** used by `mod.rs` before tokenising for the CTC aligner.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** every spoken word maps back to exactly one displayed word; a displayed word
//! with nothing speakable maps to no spoken word.

/// Spoken words of `display`, lowercase, letters and apostrophes only.
pub fn spoken(display: &str) -> Vec<String> {
    let mut out = Vec::new();
    let replaced = display.replace('&', " and ").replace('%', " percent ");
    for piece in replaced.split(|c: char| c.is_whitespace() || c == '-' || c == '/' || c == '—') {
        let folded: String = piece.chars().filter_map(fold).collect();
        let folded = folded.trim_matches('\'').to_string();
        if folded.is_empty() {
            continue;
        }
        if folded.chars().all(|c| c.is_ascii_digit()) {
            match folded.parse::<u64>() {
                Ok(n) if n < 1_000_000 => out.extend(number_words(n)),
                _ => out.extend(
                    folded
                        .chars()
                        .filter_map(|c| c.to_digit(10))
                        .map(|d| number_words(d as u64).join(" ")),
                ),
            }
        } else {
            out.push(folded.chars().filter(|c| !c.is_ascii_digit()).collect());
        }
    }
    out.retain(|w: &String| !w.is_empty());
    out
}

/// Lowercase ASCII letters, digits and apostrophes; accented letters lose their accent.
fn fold(c: char) -> Option<char> {
    let c = c.to_lowercase().next().unwrap_or(c);
    let plain = match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        '’' | '\'' => '\'',
        c => c,
    };
    (plain.is_ascii_lowercase() || plain.is_ascii_digit() || plain == '\'').then_some(plain)
}

/// English words for `n` below one million.
pub fn number_words(n: u64) -> Vec<String> {
    const ONES: [&str; 20] = [
        "zero",
        "one",
        "two",
        "three",
        "four",
        "five",
        "six",
        "seven",
        "eight",
        "nine",
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 10] = [
        "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
    ];
    fn below_thousand(n: u64, out: &mut Vec<String>) {
        if n >= 100 {
            out.push(ONES[(n / 100) as usize].to_string());
            out.push("hundred".to_string());
        }
        let rest = n % 100;
        if rest >= 20 {
            out.push(TENS[(rest / 10) as usize].to_string());
            if !rest.is_multiple_of(10) {
                out.push(ONES[(rest % 10) as usize].to_string());
            }
        } else if rest > 0 || n == 0 {
            out.push(ONES[rest as usize].to_string());
        }
    }
    let mut out = Vec::new();
    if n >= 1000 {
        below_thousand(n / 1000, &mut out);
        out.push("thousand".to_string());
        if !n.is_multiple_of(1000) {
            below_thousand(n % 1000, &mut out);
        }
    } else {
        below_thousand(n, &mut out);
    }
    out
}

#[cfg(test)]
#[path = "tests/spoken_form.rs"]
mod tests;

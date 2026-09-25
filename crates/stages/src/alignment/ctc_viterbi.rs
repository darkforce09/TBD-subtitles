//! CTC forced alignment: the single best path of a known token sequence through a CTC grid.
//!
//! **Role:** place every token of a transcript in the frames of a CTC log-probability grid, with
//! blanks allowed between tokens and required between repeats, and report each token's frames.
//!
//! **Position:** called by `mod.rs` for each alignment block; pure, on plain slices.
//!
//! **Signals and state:** one score row and one backpointer per frame and state.
//!
//! **Invariants:** tokens keep their order and never overlap; every token gets at least one
//! frame; `None` when the grid has too few frames for the tokens.

/// Frames `[start, end)` of each token, in order.
pub type TokenFrames = Vec<(usize, usize)>;

/// Align `tokens` through `log_probs` (`frames × vocab`, flattened) with blank `blank`.
pub fn align(
    log_probs: &[f32],
    vocab: usize,
    tokens: &[usize],
    blank: usize,
) -> Option<TokenFrames> {
    let frames = log_probs.len().checked_div(vocab).unwrap_or(0);
    if tokens.is_empty() {
        return Some(Vec::new());
    }
    // States: blank, t0, blank, t1, …, blank.
    let states = 2 * tokens.len() + 1;
    let label = |s: usize| {
        if s.is_multiple_of(2) {
            blank
        } else {
            tokens[s / 2]
        }
    };
    let lp = |t: usize, s: usize| log_probs[t * vocab + label(s)];
    let neg = f32::NEG_INFINITY;
    let mut score = vec![neg; states];
    let mut back = vec![0u8; frames * states];
    if frames == 0 {
        return None;
    }
    score[0] = lp(0, 0);
    if states > 1 {
        score[1] = lp(0, 1);
    }
    let mut next = vec![neg; states];
    for t in 1..frames {
        for s in 0..states {
            let mut best = score[s];
            let mut step = 0u8;
            if s >= 1 && score[s - 1] > best {
                best = score[s - 1];
                step = 1;
            }
            if s >= 2 && !s.is_multiple_of(2) && label(s) != label(s - 2) && score[s - 2] > best {
                best = score[s - 2];
                step = 2;
            }
            next[s] = if best == neg { neg } else { best + lp(t, s) };
            back[t * states + s] = step;
        }
        std::mem::swap(&mut score, &mut next);
    }
    let last = states - 1;
    let mut s = if score[last] >= score[last - 1] {
        last
    } else {
        last - 1
    };
    if score[s] == neg {
        return None;
    }
    let mut path = vec![0usize; frames];
    for t in (0..frames).rev() {
        path[t] = s;
        if t > 0 {
            s -= back[t * states + s] as usize;
        }
    }
    let mut spans = vec![(usize::MAX, 0usize); tokens.len()];
    for (t, state) in path.iter().enumerate() {
        if !state.is_multiple_of(2) {
            let span = &mut spans[state / 2];
            span.0 = span.0.min(t);
            span.1 = t + 1;
        }
    }
    spans.iter().all(|s| s.0 != usize::MAX).then_some(spans)
}

#[cfg(test)]
#[path = "tests/ctc_viterbi.rs"]
mod tests;

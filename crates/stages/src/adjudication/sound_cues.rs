//! The language model chooses which sound candidates become cues and words them.
//!
//! **Role:** show the model the candidates with the dialogue around them, window by window, and
//! keep the cues whose answer passes the checks: a known candidate id, used once, worded as one
//! bracketed lowercase phrase (proper nouns from the glossary allowed). Every song becomes a
//! music cue, worded by the model or `[music playing]`.
//!
//! **Position:** called by the sound-cue step with a `claude` model factory; the candidates come
//! from `crate::sound_events::candidates`.
//!
//! **Signals and state:** one model call per window, several windows at once.
//!
//! **Invariants:** a cue always takes its candidate's times (the model never sets one); a cue's
//! text is bracketed; no candidate yields two cues.

use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use inference::llm::LanguageModel;
use job_model::outputs::{CandidateKind, SoundCandidate, SoundCue, SoundCues};
use serde::Deserialize;
use serde_json::{Value, json};

/// Seconds of video per model call.
pub const WINDOW_S: f64 = 300.0;
/// Seconds of dialogue shown on either side of a window.
pub const CONTEXT_S: f64 = 10.0;
/// The wording of a song the model did not word.
pub const MUSIC_FALLBACK: &str = "[music playing]";

/// The rules of the sound-cue call.
pub const SYSTEM: &str = "\
You choose the sound cues of English SDH subtitles for an anime episode. The input lists sound \
candidates a detector found, one per line as `ID m:ss.d duration kind label score`, mixed in time \
order with the dialogue as `m:ss.d text`. Choose only the sounds that matter to the story or the \
mood and that a deaf viewer could not tell from the dialogue or the picture; leave most \
candidates out, never a cue for every grunt of a fight, at most a few per minute. Word each chosen \
cue in U.S. English as one short phrase in square brackets, lowercase except proper nouns, \
precise: [explosion], [crowd cheering], [gasps], [laughs nervously]. A `song` candidate is a song \
whose lyrics are not shown: always choose it and word it as one music cue, such as \
[upbeat music playing] or [theme song playing]. Never describe a sound that is not a candidate. \
Answer only with the JSON.";

/// The answer's JSON Schema.
pub fn schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "cues": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {"id": {"type": "string"}, "text": {"type": "string"}},
                    "required": ["id", "text"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["cues"],
        "additionalProperties": false
    })
}

/// One chosen candidate, as the model returns it.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Choice {
    pub id: String,
    pub text: String,
}

/// The message for one window: its candidates and the dialogue around them, in time order.
pub fn user_message(
    glossary: &[&str],
    candidates: &[&SoundCandidate],
    dialogue: &[(f64, String)],
) -> String {
    let (Some(first), Some(last)) = (candidates.first(), candidates.last()) else {
        return String::new();
    };
    let (from, to) = (first.start_s - CONTEXT_S, last.end_s + CONTEXT_S);
    let mut rows: Vec<(f64, String)> = dialogue
        .iter()
        .filter(|(t, _)| *t >= from && *t <= to)
        .map(|(t, text)| (*t, format!("{} {text}", clock(*t))))
        .collect();
    rows.extend(candidates.iter().map(|c| {
        (
            c.start_s,
            format!(
                "{} {} {:.1}s {} {} {:.2}",
                c.id,
                clock(c.start_s),
                c.end_s - c.start_s,
                kind_name(c.kind),
                c.label,
                c.peak
            ),
        )
    }));
    rows.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut message = String::from("Glossary (names, spelled right): ");
    message.push_str(&glossary.join(", "));
    message.push_str("\n\nTimeline:\n");
    for (_, row) in rows {
        message.push_str(&row);
        message.push('\n');
    }
    message
}

/// Why a choice is refused, or `Ok` when its text is a proper cue for a known candidate.
pub fn check_choice(
    choice: &Choice,
    known: &HashMap<&str, &SoundCandidate>,
    glossary: &[&str],
) -> Result<(), String> {
    if !known.contains_key(choice.id.as_str()) {
        return Err(format!("{}: no such candidate", choice.id));
    }
    let text = choice.text.trim();
    let Some(inner) = text.strip_prefix('[').and_then(|t| t.strip_suffix(']')) else {
        return Err(format!("{}: {text:?} is not in square brackets", choice.id));
    };
    if inner.trim().is_empty() || inner.contains(['[', ']']) {
        return Err(format!(
            "{}: {text:?} is not one bracketed phrase",
            choice.id
        ));
    }
    let names: HashSet<&str> = glossary.iter().flat_map(|g| g.split_whitespace()).collect();
    for word in inner.split_whitespace() {
        let bare = word
            .trim_matches(|c: char| !c.is_alphanumeric())
            .trim_end_matches("'s");
        if bare.chars().any(char::is_uppercase) && !names.contains(bare) {
            return Err(format!(
                "{}: {text:?} capitalises {bare:?}, not a glossary name",
                choice.id
            ));
        }
    }
    Ok(())
}

/// Ask `workers` models at once, window by window, and keep the checked choices; every song
/// ends up with a cue. `progress` hears `(windows done, windows)`.
pub fn choose(
    make: &(dyn Fn() -> Box<dyn LanguageModel + Send> + Sync),
    workers: usize,
    candidates: &[SoundCandidate],
    dialogue: &[(f64, String)],
    glossary: &[&str],
    progress: &(dyn Fn(usize, usize) + Sync),
) -> SoundCues {
    let windows = windows(candidates);
    let next = AtomicUsize::new(0);
    let done = AtomicUsize::new(0);
    let answers: Mutex<(Vec<Choice>, Vec<String>, f64)> = Mutex::new((Vec::new(), Vec::new(), 0.0));
    std::thread::scope(|scope| {
        for _ in 0..workers.max(1).min(windows.len().max(1)) {
            scope.spawn(|| {
                let mut model = make();
                while let Some(window) = windows.get(next.fetch_add(1, Ordering::SeqCst)) {
                    let message = user_message(glossary, window, dialogue);
                    let answer = model
                        .complete_json(SYSTEM, &message, &schema())
                        .and_then(|c| {
                            let choices: Vec<Choice> = serde_json::from_value(
                                c.json.get("cues").cloned().unwrap_or_default(),
                            )
                            .map_err(|e| {
                                inference::llm::LlmError(format!(
                                    "answer does not match the schema: {e}"
                                ))
                            })?;
                            Ok((choices, c.cost_usd.unwrap_or(0.0)))
                        });
                    if let Ok(mut all) = answers.lock() {
                        match answer {
                            Ok((choices, cost)) => {
                                all.0.extend(choices);
                                all.2 += cost;
                            }
                            Err(e) => all.1.push(format!("{}..: {e}", window[0].id)),
                        }
                    }
                    progress(done.fetch_add(1, Ordering::SeqCst) + 1, windows.len());
                }
            });
        }
    });
    let (choices, failed_calls, cost_usd) = answers.into_inner().unwrap_or_default();
    let mut result = settle(candidates, &choices, glossary);
    result.failed_calls = failed_calls;
    result.cost_usd = cost_usd;
    result
}

/// The cues from checked choices, in time order; refused choices are recorded; a song with no
/// accepted choice gets `MUSIC_FALLBACK`.
pub fn settle(candidates: &[SoundCandidate], choices: &[Choice], glossary: &[&str]) -> SoundCues {
    let known: HashMap<&str, &SoundCandidate> =
        candidates.iter().map(|c| (c.id.as_str(), c)).collect();
    let mut result = SoundCues {
        candidates: candidates.to_vec(),
        ..SoundCues::default()
    };
    let mut used = HashSet::new();
    for choice in choices {
        if let Err(reason) = check_choice(choice, &known, glossary) {
            result.refused.push(reason);
            continue;
        }
        if !used.insert(choice.id.clone()) {
            result.refused.push(format!("{}: chosen twice", choice.id));
            continue;
        }
        let c = known[choice.id.as_str()];
        result.cues.push(cue(c, choice.text.trim()));
    }
    for c in candidates.iter().filter(|c| c.kind == CandidateKind::Song) {
        if !used.contains(&c.id) {
            result.cues.push(cue(c, MUSIC_FALLBACK));
        }
    }
    result.cues.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    result
}

fn cue(c: &SoundCandidate, text: &str) -> SoundCue {
    SoundCue {
        candidate: c.id.clone(),
        kind: c.kind,
        start_s: c.start_s,
        end_s: c.end_s,
        text: text.to_string(),
    }
}

/// Candidates grouped by `WINDOW_S` of video.
fn windows(candidates: &[SoundCandidate]) -> Vec<Vec<&SoundCandidate>> {
    let mut windows: Vec<Vec<&SoundCandidate>> = Vec::new();
    for c in candidates {
        let index = (c.start_s / WINDOW_S).floor().max(0.0) as usize;
        match windows.last_mut() {
            Some(w) if (w[0].start_s / WINDOW_S).floor().max(0.0) as usize == index => w.push(c),
            _ => windows.push(vec![c]),
        }
    }
    windows
}

fn kind_name(kind: CandidateKind) -> &'static str {
    match kind {
        CandidateKind::Effect => "effect",
        CandidateKind::Voice => "voice",
        CandidateKind::Tag => "tag",
        CandidateKind::Song => "song",
    }
}

/// `m:ss.d`.
fn clock(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

#[cfg(test)]
#[path = "tests/sound_cues.rs"]
mod tests;

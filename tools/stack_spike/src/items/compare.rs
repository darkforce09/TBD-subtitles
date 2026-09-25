//! The comparison item: how far the transcripts disagree, and how each spells the series' names.
//!
//! **Role:** read every `asr.*.json` in the work folder, measure each one's word error rate
//! against the Parakeet-on-mix transcript chunk by chunk, count words during the opening song,
//! and count the Dressrosa names each transcript spells right.
//!
//! **Position:** called by `items/mod.rs` in a CPU worker, after the speech items.
//!
//! **Signals and state:** reads the transcripts; writes nothing but its notes.
//!
//! **Invariants:** there is no human reference transcript, so every rate here is a disagreement
//! between engines, not an accuracy.

use job_model::outputs::EngineTranscript;
use serde_json::{Map, Value};
use stages::diff_sheet::align::{self, Errors};

use super::Outcome;
use crate::context::Context;

/// The transcript every other one is compared with.
const REFERENCE: &str = "asr.parakeet.mix.json";
/// Where the opening song ends.
const OPENING_END_S: f64 = 148.0;
/// Names that occur in Dressrosa 11, from the One Pace reference subtitles, and the arc's cast.
const NAMES: [&str; 24] = [
    "Doflamingo",
    "Rebecca",
    "Lucy",
    "Luffy",
    "Riku",
    "Garp",
    "Ace",
    "Sabo",
    "Marineford",
    "Blackbeard",
    "Colosseum",
    "Corrida",
    "Kyros",
    "Scarlett",
    "Viola",
    "Dressrosa",
    "Bartolomeo",
    "Cavendish",
    "Diamante",
    "Zoro",
    "Law",
    "Usopp",
    "Franky",
    "Tontatta",
];

pub(crate) fn run(ctx: &Context) -> anyhow::Result<Outcome> {
    let reference: EngineTranscript = ctx.read_json(REFERENCE)?;
    let mut outcome = Outcome::default();
    let mut names = std::fs::read_dir(&ctx.work)?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("asr.") && n.ends_with(".json"))
        .collect::<Vec<_>>();
    names.sort();
    for name in names {
        let transcript: EngineTranscript = ctx.read_json(&name)?;
        let key = name
            .trim_start_matches("asr.")
            .trim_end_matches(".json")
            .to_string();
        let mut errors = Errors::default();
        for (a, b) in reference.chunks.iter().zip(&transcript.chunks) {
            let a = align::normalise_all(a.words.iter().map(|w| w.text.as_str()));
            let b = align::normalise_all(b.words.iter().map(|w| w.text.as_str()));
            errors.add(align::errors(&a, &b));
        }
        let mut entry = Map::new();
        entry.insert("words".into(), transcript.words().count().into());
        entry.insert("wer_vs_parakeet_mix".into(), errors.rate().into());
        entry.insert(
            "words_in_opening".into(),
            transcript
                .words()
                .filter(|w| w.start_s < OPENING_END_S)
                .count()
                .into(),
        );
        let text = transcript.text();
        let hits: Map<String, Value> = NAMES
            .iter()
            .map(|n| (n.to_string(), Value::from(count_word(&text, n))))
            .filter(|(_, v)| v.as_u64() != Some(0))
            .collect();
        entry.insert("names".into(), Value::Object(hits));
        outcome.note(&key, Value::Object(entry));
    }
    Ok(outcome)
}

/// Whole-word occurrences of `name`, case as written.
fn count_word(text: &str, name: &str) -> usize {
    text.match_indices(name)
        .filter(|(i, _)| {
            let before = text[..*i].chars().next_back();
            let after = text[i + name.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
        .count()
}

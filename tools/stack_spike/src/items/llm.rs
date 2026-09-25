//! The diff-sheet and language-model items: the sheet built from Parakeet and Whisper on the mix,
//! and each backend's adjudication of it, checked.
//!
//! **Role:** build the diff sheet; run a backend over it; record time, calls, tokens, cost, the
//! checks' findings, the flags used, and how the glossary names changed against Parakeet's text.
//!
//! **Position:** called by `items/mod.rs`; uses `stages::diff_sheet` and `stages::adjudication`
//! with an `inference::llm` backend.
//!
//! **Signals and state:** reads `asr.parakeet.mix.json` and `asr.whisper.mix.json`; writes
//! `sheet.txt`, `sheet.json` and `adjudicated.<backend>.json`.
//!
//! **Invariants:** every backend answers the same sheet with the same glossary and rules.

use std::collections::BTreeMap;
use std::time::Instant;

use inference::llm::LanguageModel;
use inference::llm::claude_cli::ClaudeCli;
use job_model::outputs::EngineTranscript;
use serde_json::json;
use stages::adjudication::{self, Adjudication, checks};
use stages::diff_sheet::sheet::{self, Utterance};

use super::{Outcome, since};
use crate::context::Context;

/// The series glossary: names and terms of the Dressrosa arc.
pub(crate) const GLOSSARY: &[&str] = &[
    "Luffy",
    "Lucy",
    "Zoro",
    "Nami",
    "Usopp",
    "Sanji",
    "Chopper",
    "Robin",
    "Franky",
    "Brook",
    "Law",
    "Trafalgar Law",
    "Doflamingo",
    "Donquixote",
    "Rebecca",
    "Kyros",
    "Scarlett",
    "Viola",
    "Riku",
    "Dold",
    "Dressrosa",
    "Colosseum",
    "Corrida",
    "Bartolomeo",
    "Cavendish",
    "Diamante",
    "Trebol",
    "Pica",
    "Sugar",
    "Senor Pink",
    "Gladius",
    "Machvise",
    "Dellinger",
    "Lao G",
    "Kanjuro",
    "Kin'emon",
    "Momonosuke",
    "Fujitora",
    "Issho",
    "Sabo",
    "Ace",
    "Garp",
    "Marineford",
    "Blackbeard",
    "Tontatta",
    "Leo",
    "Mansherry",
    "Birdcage",
    "Straw Hat",
    "Flame-Flame Fruit",
    "Mera Mera",
    "Haki",
    "Kaido",
    "Punk Hazard",
    "Caesar",
    "Sengoku",
];

/// Build and write the diff sheet.
pub(crate) fn diff_sheet(ctx: &Context) -> anyhow::Result<Outcome> {
    let started = Instant::now();
    let backbone: EngineTranscript = ctx.read_json("asr.parakeet.mix.json")?;
    let whisper: EngineTranscript = ctx.read_json("asr.whisper.mix.json")?;
    let utterances = sheet::build(&backbone, &[&whisper], &["P", "W"]);
    let text: String = utterances.iter().map(|u| format!("{}\n", u.line)).collect();
    std::fs::write(ctx.path("sheet.txt"), text)?;
    ctx.write_json("sheet.json", &utterances)?;
    let words: usize = utterances.iter().map(|u| u.words.len()).sum();
    let locked: usize = utterances
        .iter()
        .flat_map(|u| &u.locked)
        .filter(|l| **l)
        .count();
    let mut outcome = Outcome {
        audio_s: ctx.probe()?.duration_s,
        process_s: since(started),
        ..Outcome::default()
    };
    outcome.note("utterances", utterances.len());
    outcome.note("words", words);
    outcome.note("locked_share", locked as f64 / words.max(1) as f64);
    outcome.note(
        "utterances_with_disagreement",
        utterances
            .iter()
            .filter(|u| u.locked.iter().any(|l| !l))
            .count(),
    );
    Ok(outcome)
}

/// `claude -p` processes run at once.
const WORKERS: usize = 8;

/// Adjudicate with `claude -p`, `WORKERS` processes at once.
pub(crate) fn claude(ctx: &Context) -> anyhow::Result<Outcome> {
    let sheet: Vec<Utterance> = ctx.read_json("sheet.json")?;
    let cwd = ctx.path("claude-cwd");
    let make =
        || -> Box<dyn LanguageModel + Send> { Box::new(ClaudeCli::new("sonnet", cwd.clone())) };
    let started = Instant::now();
    let result = adjudication::adjudicate_concurrently(&make, WORKERS, &sheet, GLOSSARY);
    report(ctx, "claude", &sheet, result, since(started), 0.0)
}

/// Write the answer and its checks; turn them into notes.
pub(crate) fn report(
    ctx: &Context,
    backend: &str,
    sheet: &[Utterance],
    result: Adjudication,
    process_s: f64,
    load_s: f64,
) -> anyhow::Result<Outcome> {
    let findings = checks::check(sheet, &result.lines, GLOSSARY);
    ctx.write_json(
        &format!("adjudicated.{backend}.json"),
        &json!({"lines": result.lines, "findings": findings, "failed_calls": result.failed_calls}),
    )?;
    let mut flags: BTreeMap<String, usize> = BTreeMap::new();
    for line in &result.lines {
        for f in &line.f {
            *flags.entry(f.clone()).or_default() += 1;
        }
    }
    let backbone_text: String = sheet
        .iter()
        .flat_map(|u| &u.words)
        .map(|w| format!("{} ", w.text))
        .collect();
    let output_text: String = result.lines.iter().map(|l| format!("{} ", l.t)).collect();
    let mut names = BTreeMap::new();
    for name in GLOSSARY {
        let (before, after) = (count(&backbone_text, name), count(&output_text, name));
        if before != after {
            names.insert(name.to_string(), format!("{before} -> {after}"));
        }
    }
    let mut outcome = Outcome {
        audio_s: ctx.probe()?.duration_s,
        load_s,
        process_s,
        ..Outcome::default()
    };
    outcome.note("utterances", sheet.len());
    outcome.note("lines_returned", result.lines.len());
    outcome.note("calls", result.calls);
    outcome.note("failed_calls", result.failed_calls.len());
    outcome.note("input_tokens", result.input_tokens);
    outcome.note("output_tokens", result.output_tokens);
    outcome.note("cost_usd", result.cost_usd);
    outcome.note("missing_ids", findings.missing_ids.len());
    outcome.note("duplicate_ids", findings.duplicate_ids.len());
    outcome.note("novel_words", findings.novel.len());
    outcome.note("removed_locked_words", findings.removed_locked.len());
    outcome.note("too_fast_lines", findings.too_fast.len());
    outcome.note("flags", json!(flags));
    outcome.note("name_count_changes", json!(names));
    outcome.note(
        "changed_lines",
        result
            .lines
            .iter()
            .filter(|l| {
                sheet.iter().find(|u| u.id == l.id).is_some_and(|u| {
                    let before: Vec<&str> = u.words.iter().map(|w| w.text.as_str()).collect();
                    before.join(" ") != l.t
                })
            })
            .count(),
    );
    Ok(outcome)
}

fn count(text: &str, name: &str) -> usize {
    text.match_indices(name)
        .filter(|(i, _)| {
            let before = text[..*i].chars().next_back();
            let after = text[i + name.len()..].chars().next();
            !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
        })
        .count()
}

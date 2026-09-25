//! The sound-event item: CED-base over the background stem (effects, music) and the vocal stem
//! (non-speech voices, singing), and how much music each separator leaves in its vocal stem.
//!
//! **Role:** score both RoFormer stems in 2 s windows every 0.5 s, cut events for the subtitle
//! classes, and compare the mean Music score of both separators' vocal stems outside the opening.
//!
//! **Position:** called by `items/mod.rs` inside a GPU worker; uses `stages::sound_events` and
//! `inference::onnx::ced`.
//!
//! **Signals and state:** reads the stems; writes `sound_events.json`.
//!
//! **Invariants:** music left in a vocal stem is measured on the same windows for both separators.

use std::collections::BTreeMap;
use std::time::Instant;

use inference::onnx::Device;
use inference::onnx::ced::{self, Ced};
use stages::sound_events::{self, Windowing, classes};

use super::{Outcome, since};
use crate::context::Context;

/// Where the opening song ends; music measured after it is music under dialogue.
const OPENING_END_S: f64 = 148.0;

pub(crate) fn run(ctx: &Context) -> anyhow::Result<Outcome> {
    let duration = ctx.probe()?.duration_s;
    let load = Instant::now();
    let mut tagger = Ced::open(&ctx.models.join(ced::MODEL), Device::Cuda)?;
    let mut outcome = Outcome {
        audio_s: duration,
        load_s: since(load),
        ..Outcome::default()
    };
    let windowing = Windowing::default();
    let music = classes::index_of("Music").ok_or_else(|| anyhow::anyhow!("no Music class"))?;
    let singing =
        classes::index_of("Singing").ok_or_else(|| anyhow::anyhow!("no Singing class"))?;
    let mut events = Vec::new();
    let mut scoring_s = 0.0;
    for (stem, file, table) in [
        (
            "background",
            "background_16k.roformer.f32",
            classes::BACKGROUND,
        ),
        ("vocals", "vocals_16k.roformer.f32", classes::VOCALS),
    ] {
        let started = Instant::now();
        let rows = sound_events::score_stem(&mut tagger, &ctx.path(file), duration, &windowing)
            .map_err(anyhow::Error::msg)?;
        scoring_s += since(started);
        for rule in classes::rules(table).map_err(anyhow::Error::msg)? {
            events.extend(sound_events::events(&rows, &rule, &windowing, stem));
        }
        if stem == "vocals" {
            outcome.note(
                "roformer_vocals_music_after_opening",
                sound_events::mean_score(&rows, music, &windowing, OPENING_END_S, duration),
            );
            outcome.note(
                "roformer_vocals_singing_in_opening",
                sound_events::mean_score(&rows, singing, &windowing, 0.0, OPENING_END_S),
            );
        }
    }
    outcome.process_s = scoring_s;
    let started = Instant::now();
    let mdx_rows = sound_events::score_stem(
        &mut tagger,
        &ctx.path("vocals_16k.mdx.f32"),
        duration,
        &windowing,
    )
    .map_err(anyhow::Error::msg)?;
    outcome.note("mdx_scoring_s", since(started));
    outcome.note(
        "mdx_vocals_music_after_opening",
        sound_events::mean_score(&mdx_rows, music, &windowing, OPENING_END_S, duration),
    );
    events.sort_by(|a, b| a.start_s.total_cmp(&b.start_s));
    let mut per_label: BTreeMap<String, (usize, f64)> = BTreeMap::new();
    for e in &events {
        let entry = per_label
            .entry(format!("{}:{}", e.stem, e.label))
            .or_default();
        entry.0 += 1;
        entry.1 += e.end_s - e.start_s;
    }
    for (label, (count, seconds)) in per_label {
        outcome.note(
            &format!("events {label}"),
            format!("{count} events, {seconds:.0} s"),
        );
    }
    ctx.write_json("sound_events.json", &events)?;
    Ok(outcome)
}

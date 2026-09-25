//! The voice-activity item: earshot on the mix and on both vocal stems, each with its chunk plan.

use std::time::Instant;

use job_model::outputs::TimeSpan;
use stages::vad::{self, VadSettings};

use super::{Outcome, since};
use crate::context::Context;

/// The opening song of a Dressrosa episode (the playlist skips to 148 s).
const OPENING: TimeSpan = TimeSpan {
    start_s: 0.0,
    end_s: 148.0,
};

/// The inputs compared: the mix, then each separator's vocal stem.
pub(crate) const INPUTS: [(&str, &str); 3] = [
    ("mix", "mix_16k.f32"),
    ("mdx", "vocals_16k.mdx.f32"),
    ("roformer", "vocals_16k.roformer.f32"),
];

pub(crate) fn run(ctx: &Context) -> anyhow::Result<Outcome> {
    let probe = ctx.probe()?;
    let settings = VadSettings::default();
    let mut outcome = Outcome {
        audio_s: probe.duration_s,
        ..Outcome::default()
    };
    let mut total_s = 0.0;
    for (name, file) in INPUTS {
        let started = Instant::now();
        let scores = vad::score_file(&ctx.path(file))?;
        let score_s = since(started);
        let plan = vad::plan(&scores, probe.duration_s, &settings);
        total_s += since(started);
        ctx.write_json(&format!("vad.{name}.json"), &plan)?;
        let lengths: Vec<f64> = plan.chunks.iter().map(TimeSpan::duration_s).collect();
        outcome.note(&format!("{name}_score_s"), score_s);
        outcome.note(&format!("{name}_speech_s"), plan.speech_s());
        outcome.note(
            &format!("{name}_speech_share"),
            plan.speech_s() / probe.duration_s,
        );
        outcome.note(&format!("{name}_regions"), plan.regions.len());
        outcome.note(&format!("{name}_chunks"), plan.chunks.len());
        outcome.note(
            &format!("{name}_chunk_s_min_max"),
            vec![
                lengths.iter().copied().fold(f64::INFINITY, f64::min),
                lengths.iter().copied().fold(0.0, f64::max),
            ],
        );
        outcome.note(
            &format!("{name}_speech_in_opening_s"),
            plan.speech_within(&OPENING),
        );
    }
    outcome.process_s = total_s / INPUTS.len() as f64;
    Ok(outcome)
}

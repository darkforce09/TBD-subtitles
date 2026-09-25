//! The speech recognition items: an engine over the shared chunk plan, on the mix or a stem.
//!
//! **Role:** open one engine, run it over every chunk of the RoFormer stem's chunk plan on the
//! chosen input, and record the model load time, the processing time and word counts.
//!
//! **Position:** called by `items/mod.rs` inside a GPU worker; uses `stages::asr`.
//!
//! **Signals and state:** reads `vad.roformer.json` and the input's `.f32` file; writes
//! `asr.<engine>.<input>.json` in the work folder.
//!
//! **Invariants:** every engine and input uses the same chunk plan, so transcripts line up chunk
//! by chunk for comparison.

use std::time::Instant;

use inference::onnx::Device;
use inference::onnx::parakeet_tdt::{self, ParakeetTdt};
use job_model::outputs::SpeechPlan;
use stages::asr::{self, SpeechEngine};

use super::{Outcome, since};
use crate::context::Context;

/// The chunk plan every engine transcribes.
pub(crate) const PLAN: &str = "vad.roformer.json";

/// The audio an engine hears.
#[derive(Clone, Copy)]
pub(crate) enum Input {
    Mix,
    Mdx,
    Roformer,
}

impl Input {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Input::Mix => "mix",
            Input::Mdx => "mdx",
            Input::Roformer => "roformer",
        }
    }

    fn file(self) -> &'static str {
        match self {
            Input::Mix => "mix_16k.f32",
            Input::Mdx => "vocals_16k.mdx.f32",
            Input::Roformer => "vocals_16k.roformer.f32",
        }
    }
}

/// Run Parakeet-TDT over the plan on `input`.
pub(crate) fn parakeet(ctx: &Context, input: Input) -> anyhow::Result<Outcome> {
    let load = Instant::now();
    let mut engine = ParakeetTdt::open(&ctx.models.join(parakeet_tdt::MODEL), Device::Cuda)?;
    let load_s = since(load);
    run(ctx, &mut engine, "parakeet", input, load_s)
}

/// Run `engine` over the plan and write its transcript.
pub(crate) fn run(
    ctx: &Context,
    engine: &mut dyn SpeechEngine,
    short_name: &str,
    input: Input,
    load_s: f64,
) -> anyhow::Result<Outcome> {
    let plan: SpeechPlan = ctx.read_json(PLAN)?;
    let started = Instant::now();
    let transcript = asr::transcribe_plan(
        engine,
        &ctx.path(input.file()),
        input.name(),
        &plan,
        |done, all| {
            if done % 10 == 0 || done == all {
                println!("chunk {done}/{all}");
            }
        },
    )
    .map_err(anyhow::Error::msg)?;
    let process_s = since(started);
    ctx.write_json(
        &format!("asr.{short_name}.{}.json", input.name()),
        &transcript,
    )?;
    let speech_s: f64 = plan.chunks.iter().map(|c| c.duration_s()).sum();
    // Speed and projections count video time; the engine hears only the chunks.
    let mut outcome = Outcome {
        audio_s: ctx.probe()?.duration_s,
        load_s,
        process_s,
        ..Outcome::default()
    };
    outcome.note("engine", transcript.engine.clone());
    outcome.note("chunks", plan.chunks.len());
    outcome.note("chunked_audio_s", speech_s);
    outcome.note("words", transcript.words().count());
    Ok(outcome)
}

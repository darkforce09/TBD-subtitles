//! The ggml items: Whisper over the shared chunk plan, and the Qwen3 aligner (in `align.rs`).
//!
//! **Role:** open a Whisper model through CrispASR and run it over the RoFormer stem's chunk plan
//! on the mix or the RoFormer vocal stem, writing `asr.<engine>.<input>.json`.
//!
//! **Position:** called by `main.rs`; uses `stages::asr` and `inference::ggml::crispasr`.
//!
//! **Signals and state:** reads `vad.roformer.json` and the input's `.f32` file in the work folder.
//!
//! **Invariants:** the chunk plan is the one the ONNX engines use, so transcripts line up.

use std::path::Path;
#[cfg(feature = "crispasr")]
use std::time::Instant;

use crate::report::Outcome;

/// The ggml items `stack-spike` hands to this binary; names match its own.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Item {
    #[value(name = "asr-whisper-mix")]
    LargeV3OnMix,
    #[value(name = "asr-whisper-roformer")]
    LargeV3OnRoformer,
    #[value(name = "asr-whisper-turbo-mix")]
    TurboOnMix,
    #[value(name = "align-qwen3")]
    Qwen3Aligner,
}

impl Item {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Item::LargeV3OnMix => "asr-whisper-mix",
            Item::LargeV3OnRoformer => "asr-whisper-roformer",
            Item::TurboOnMix => "asr-whisper-turbo-mix",
            Item::Qwen3Aligner => "align-qwen3",
        }
    }
}

/// Run `item` over the work folder `work`.
pub(crate) fn run(item: Item, work: &Path) -> anyhow::Result<Outcome> {
    #[cfg(feature = "crispasr")]
    {
        let (model, file, short, input, audio) = match item {
            Item::Qwen3Aligner => return crate::align::run(work),
            Item::LargeV3OnMix => (
                "whisper-large-v3",
                "ggml-large-v3.bin",
                "whisper",
                "mix",
                "mix_16k.f32",
            ),
            Item::LargeV3OnRoformer => (
                "whisper-large-v3",
                "ggml-large-v3.bin",
                "whisper",
                "roformer",
                "vocals_16k.roformer.f32",
            ),
            Item::TurboOnMix => (
                "whisper-large-v3-turbo",
                "ggml-large-v3-turbo-q8_0.bin",
                "whisper-turbo",
                "mix",
                "mix_16k.f32",
            ),
        };
        let models = inference::model_store::models_dir()?;
        let load = Instant::now();
        let mut engine =
            inference::ggml::crispasr::Whisper::open(&models.join(model).join(file), model, 8)
                .map_err(anyhow::Error::msg)?;
        let load_s = load.elapsed().as_secs_f64();
        let plan: job_model::outputs::SpeechPlan =
            serde_json::from_str(&std::fs::read_to_string(work.join("vad.roformer.json"))?)?;
        let started = Instant::now();
        let transcript = stages::asr::transcribe_plan(
            &mut engine,
            &work.join(audio),
            input,
            &plan,
            |done, all| {
                if done % 10 == 0 || done == all {
                    println!("chunk {done}/{all}");
                }
            },
        )
        .map_err(anyhow::Error::msg)?;
        let process_s = started.elapsed().as_secs_f64();
        let out = work.join(format!("asr.{short}.{input}.json"));
        std::fs::write(&out, serde_json::to_vec_pretty(&transcript)?)?;
        let chunked_s: f64 = plan.chunks.iter().map(|c| c.duration_s()).sum();
        let probe: job_model::outputs::ProbeResult =
            serde_json::from_str(&std::fs::read_to_string(work.join("probe.json"))?)?;
        // Speed and projections count video time; the engine hears only the chunks.
        let mut outcome = Outcome {
            audio_s: probe.duration_s,
            load_s,
            process_s,
            ..Outcome::default()
        };
        outcome.note("engine", transcript.engine.clone());
        outcome.note("chunks", plan.chunks.len());
        outcome.note("chunked_audio_s", chunked_s);
        outcome.note("words", transcript.words().count());
        Ok(outcome)
    }
    #[cfg(not(feature = "crispasr"))]
    {
        let _ = work;
        anyhow::bail!(
            "stack-spike-ggml was built without the `crispasr` feature, so {} cannot run",
            item.name()
        )
    }
}

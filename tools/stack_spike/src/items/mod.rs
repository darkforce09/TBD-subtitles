//! The stack items the spike measures, each run inside its own worker process.
//!
//! **Role:** list the items in run order, say which need the GPU, and dispatch each to its module.
//!
//! **Position:** `Item::run` is called by the worker (`measure/worker.rs`); `needs_gpu` by the
//! parent (`measure/mod.rs`).
//!
//! **Signals and state:** none; each item reads the video and the work folder.
//!
//! **Invariants:** an item that reads another item's output comes after it in `Item::ALL`.

mod align;
mod asr;
mod compare;
mod decode;
mod separate;
mod shots;
mod sound_events;
mod vad;

use std::time::Instant;

use serde_json::{Map, Value};

use crate::context::Context;

/// One measurable piece of the stack, in the order `run all` takes them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum Item {
    /// ffprobe, then FFmpeg decoding 16 kHz mono to a file and 44.1 kHz stereo to nothing.
    Decode,
    /// FFmpeg's scdet shot-change scan on a 480-pixel copy, on the CPU and with NVDEC.
    Shots,
    /// Vocal separation with UVR MDX-Net Voc_FT on CUDA.
    SeparateMdx,
    /// Vocal separation with Mel-Band RoFormer on CUDA.
    SeparateRoformer,
    /// earshot voice activity and the chunk plan, on the mix and both vocal stems.
    Vad,
    /// Parakeet-TDT-0.6B-v2 on the mix.
    AsrParakeetMix,
    /// Parakeet-TDT-0.6B-v2 on the MDX-Net vocal stem.
    AsrParakeetMdx,
    /// Parakeet-TDT-0.6B-v2 on the RoFormer vocal stem.
    AsrParakeetRoformer,
    /// Whisper large-v3 (CrispASR, ggml) on the mix.
    AsrWhisperMix,
    /// Whisper large-v3 on the RoFormer vocal stem.
    AsrWhisperRoformer,
    /// Whisper large-v3-turbo (8-bit) on the mix.
    AsrWhisperTurboMix,
    /// Word disagreement between the transcripts, and name spellings.
    AsrCompare,
    /// CTC Viterbi forced alignment over Parakeet-CTC on the vocal stem.
    AlignCtc,
    /// CED-base sound events on both stems, and music left in each vocal stem.
    SoundEvents,
    /// The Qwen3 forced aligner (CrispASR, ggml) on the vocal stem.
    AlignQwen3,
}

impl Item {
    pub(crate) const ALL: &[Item] = &[
        Item::Decode,
        Item::Shots,
        Item::SeparateMdx,
        Item::SeparateRoformer,
        Item::Vad,
        Item::AsrParakeetMix,
        Item::AsrParakeetMdx,
        Item::AsrParakeetRoformer,
        Item::AsrWhisperMix,
        Item::AsrWhisperRoformer,
        Item::AsrWhisperTurboMix,
        Item::AsrCompare,
        Item::AlignCtc,
        Item::AlignQwen3,
        Item::SoundEvents,
    ];

    /// The name used on the command line and in result files.
    pub(crate) fn name(self) -> String {
        clap::ValueEnum::to_possible_value(&self)
            .map(|v| v.get_name().to_string())
            .unwrap_or_default()
    }

    /// Whether the item runs a model on the GPU, and so needs the VRAM budget free.
    pub(crate) fn needs_gpu(self) -> bool {
        match self {
            Item::Decode | Item::Shots | Item::Vad | Item::AsrCompare => false,
            Item::SeparateMdx | Item::SeparateRoformer => true,
            Item::AsrParakeetMix | Item::AsrParakeetMdx | Item::AsrParakeetRoformer => true,
            Item::AsrWhisperMix | Item::AsrWhisperRoformer | Item::AsrWhisperTurboMix => true,
            Item::AlignCtc | Item::AlignQwen3 | Item::SoundEvents => true,
        }
    }

    /// Whether the item runs a ggml model, and so runs in the `stack-spike-ggml` binary: ggml and
    /// ONNX Runtime corrupt each other's heap when loaded into one process.
    pub(crate) fn ggml(self) -> bool {
        matches!(
            self,
            Item::AsrWhisperMix
                | Item::AsrWhisperRoformer
                | Item::AsrWhisperTurboMix
                | Item::AlignQwen3
        )
    }

    /// Run the item in this (worker) process.
    pub(crate) fn run(self, ctx: &Context) -> anyhow::Result<Outcome> {
        match self {
            Item::Decode => decode::run(ctx),
            Item::Shots => shots::run(ctx),
            Item::SeparateMdx => separate::run(ctx, separate::Separator::MdxVocFt),
            Item::SeparateRoformer => separate::run(ctx, separate::Separator::MelRoformer),
            Item::Vad => vad::run(ctx),
            Item::AsrParakeetMix => asr::parakeet(ctx, asr::Input::Mix),
            Item::AsrParakeetMdx => asr::parakeet(ctx, asr::Input::Mdx),
            Item::AsrParakeetRoformer => asr::parakeet(ctx, asr::Input::Roformer),
            Item::AlignCtc => align::run(ctx),
            Item::SoundEvents => sound_events::run(ctx),
            Item::AsrWhisperMix
            | Item::AsrWhisperRoformer
            | Item::AsrWhisperTurboMix
            | Item::AlignQwen3 => {
                anyhow::bail!("{} runs in stack-spike-ggml, not here", self.name())
            }
            Item::AsrCompare => compare::run(ctx),
        }
    }
}

/// What an item measured about itself.
#[derive(Debug, Default)]
pub(crate) struct Outcome {
    /// Seconds of audio the item processed, for the speed against realtime.
    pub(crate) audio_s: f64,
    /// Seconds spent loading models or starting up.
    pub(crate) load_s: f64,
    /// Seconds spent processing the audio.
    pub(crate) process_s: f64,
    /// Item-specific numbers and quality notes.
    pub(crate) notes: Map<String, Value>,
}

impl Outcome {
    pub(crate) fn note(&mut self, key: &str, value: impl Into<Value>) {
        self.notes.insert(key.to_string(), value.into());
    }
}

/// Seconds since `start`.
pub(crate) fn since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64()
}

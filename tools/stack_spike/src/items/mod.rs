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

mod decode;
mod separate;
mod shots;

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
}

impl Item {
    pub(crate) const ALL: &[Item] = &[
        Item::Decode,
        Item::Shots,
        Item::SeparateMdx,
        Item::SeparateRoformer,
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
            Item::Decode | Item::Shots => false,
            Item::SeparateMdx | Item::SeparateRoformer => true,
        }
    }

    /// Run the item in this (worker) process.
    pub(crate) fn run(self, ctx: &Context) -> anyhow::Result<Outcome> {
        match self {
            Item::Decode => decode::run(ctx),
            Item::Shots => shots::run(ctx),
            Item::SeparateMdx => separate::run(ctx, separate::Separator::MdxVocFt),
            Item::SeparateRoformer => separate::run(ctx, separate::Separator::MelRoformer),
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

//! The separation items: MDX-Net Voc_FT and Mel-Band RoFormer over the whole track.
//!
//! **Role:** run one separator through the separation stage, record its model and STFT time and
//! the stems' levels, and write 30-second WAV excerpts to listen to.
//!
//! **Position:** called by `items/mod.rs` inside a GPU worker.
//!
//! **Signals and state:** writes `vocals_16k.<suffix>.f32`, `background_16k.<suffix>.f32` and the
//! excerpts in the work folder.
//!
//! **Invariants:** each separator writes its own stems, so both can be compared.

use std::path::Path;
use std::time::{Duration, Instant};

use inference::onnx::separation::{MdxNet, MelRoformer, mdx_net};
use media_io::pcm_stream::F32FileReader;
use media_io::probe;
use stages::separation::{self, SeparationRequest};

use super::{Outcome, since};
use crate::context::Context;
use crate::wav;

const DEADLINE: Duration = Duration::from_secs(3 * 3600);
/// Windows per MDX-Net call: larger batches measured no faster and grew the VRAM arena past 5 GB.
const MDX_BATCH: usize = 1;
/// Excerpt starts in seconds: inside the opening song, and two stretches of the episode.
const EXCERPTS: [f64; 3] = [60.0, 600.0, 1200.0];
const EXCERPT_S: f64 = 30.0;

/// Which separator an item runs.
#[derive(Clone, Copy)]
pub(crate) enum Separator {
    MdxVocFt,
    MelRoformer,
}

impl Separator {
    /// The stem file suffix: `vocals_16k.<suffix>.f32`.
    pub(crate) fn suffix(self) -> &'static str {
        match self {
            Separator::MdxVocFt => "mdx",
            Separator::MelRoformer => "roformer",
        }
    }
}

pub(crate) fn run(ctx: &Context, separator: Separator) -> anyhow::Result<Outcome> {
    let probe = ctx.probe()?;
    let track = probe::english_track(&probe)?.clone();
    let vocals = ctx.path(&format!("vocals_16k.{}.f32", separator.suffix()));
    let background = ctx.path(&format!("background_16k.{}.f32", separator.suffix()));
    let request = SeparationRequest {
        programs: &ctx.programs,
        video: &ctx.video,
        audio_position: track.audio_position,
        deadline: DEADLINE,
        vocals_16k: &vocals,
        background_16k: &background,
        duration_s: probe.duration_s,
        progress: &|_, _| {},
    };
    let mut outcome = Outcome {
        audio_s: probe.duration_s,
        ..Outcome::default()
    };
    let load = Instant::now();
    let (summary, model_s, stft_s) = match separator {
        Separator::MdxVocFt => {
            let model = MdxNet::open(
                &ctx.model_file("mdx-net-voc-ft", "UVR-MDX-NET-Voc_FT.onnx"),
                mdx_net::VOC_FT,
                MDX_BATCH,
            )?;
            outcome.load_s = since(load);
            let started = Instant::now();
            let (summary, model) = separation::separate(model, &request)?;
            outcome.process_s = since(started);
            (summary, model.model_s, model.stft_s)
        }
        Separator::MelRoformer => {
            let model = MelRoformer::open(&ctx.model_file(
                "mel-band-roformer-vocals",
                "syhft_core_folded_fp16_webgpu.onnx",
            ))?;
            outcome.load_s = since(load);
            let started = Instant::now();
            let (summary, model) = separation::separate(model, &request)?;
            outcome.process_s = since(started);
            (summary, model.model_s, model.stft_s)
        }
    };
    outcome.note("model_s", model_s);
    if let Separator::MdxVocFt = separator {
        outcome.note("batch", MDX_BATCH);
    }
    outcome.note("stft_s", stft_s);
    outcome.note("decode_wait_s", summary.decode_s);
    outcome.note("frames_44k", summary.frames_44k);
    outcome.note("samples_16k", summary.samples_16k);
    outcome.note("vocal_rms", rms_of(&vocals)?);
    outcome.note("background_rms", rms_of(&background)?);
    outcome.note("mix_rms", rms_of(&ctx.path("mix_16k.f32"))?);
    for start in EXCERPTS {
        for (stem, path) in [("vocals", &vocals), ("background", &background)] {
            let name = format!(
                "excerpt_{:04}s_{stem}.{}.wav",
                start as u32,
                separator.suffix()
            );
            wav::write_excerpt(path, start, EXCERPT_S, &ctx.path(&name))?;
        }
        let mix = format!("excerpt_{:04}s_mix.wav", start as u32);
        wav::write_excerpt(&ctx.path("mix_16k.f32"), start, EXCERPT_S, &ctx.path(&mix))?;
    }
    Ok(outcome)
}

/// The RMS level of a whole 16 kHz stem.
fn rms_of(path: &Path) -> anyhow::Result<f64> {
    let mut reader = F32FileReader::open(path, 1 << 16)?;
    let (mut sum, mut count) = (0f64, 0u64);
    while let Some(chunk) = reader.next_chunk()? {
        sum += chunk.iter().map(|s| (*s as f64) * (*s as f64)).sum::<f64>();
        count += chunk.len() as u64;
    }
    Ok(if count == 0 {
        0.0
    } else {
        (sum / count as f64).sqrt()
    })
}

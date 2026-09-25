//! The decode item: ffprobe, 16 kHz mono to `mix_16k.f32`, and a 44.1 kHz stereo pass.

use std::time::{Duration, Instant};

use media_io::pcm_stream::{PcmFormat, PcmRequest, PcmStream, write_f32_file};
use media_io::probe;

use super::{Outcome, since};
use crate::context::Context;

/// Frames per chunk: one second of audio at either rate is well inside the bounded channel.
const CHUNK_FRAMES: usize = 16_000;
const DECODE_DEADLINE: Duration = Duration::from_secs(3600);

pub(crate) fn run(ctx: &Context) -> anyhow::Result<Outcome> {
    let mut outcome = Outcome::default();
    let start = Instant::now();
    let probe = probe::probe(&ctx.programs, &ctx.video)?;
    let track = probe::english_track(&probe)?.clone();
    ctx.write_json("probe.json", &probe)?;
    outcome.load_s = since(start);
    outcome.audio_s = probe.duration_s;
    outcome.note("probe_s", outcome.load_s);
    outcome.note("audio_track", track.index);
    outcome.note(
        "fps",
        probe.video.as_ref().and_then(|v| v.fps()).unwrap_or(0.0),
    );

    let request = |format| PcmRequest {
        video: &ctx.video,
        audio_position: track.audio_position,
        format,
        window: None,
        chunk_frames: CHUNK_FRAMES,
        deadline: DECODE_DEADLINE,
    };
    let mono = Instant::now();
    let stream = PcmStream::open(&ctx.programs, &request(PcmFormat::MONO_16K))?;
    let samples = write_f32_file(stream, &ctx.path("mix_16k.f32"))?;
    let mono_s = since(mono);
    outcome.note("mono_16k_s", mono_s);
    outcome.note("mono_16k_samples", samples);
    outcome.note("mono_16k_mib", samples as f64 * 4.0 / 1_048_576.0);

    let stereo = Instant::now();
    let mut stream = PcmStream::open(&ctx.programs, &request(PcmFormat::STEREO_44K))?;
    let mut stereo_samples = 0u64;
    let mut peak = 0f32;
    while let Some(chunk) = stream.next_chunk() {
        let chunk = chunk?;
        peak = chunk.iter().fold(peak, |p, s| p.max(s.abs()));
        stereo_samples += chunk.len() as u64;
    }
    stream.finish()?;
    let stereo_s = since(stereo);
    outcome.note("stereo_44k_s", stereo_s);
    outcome.note("stereo_44k_samples", stereo_samples);
    outcome.note("stereo_44k_peak", peak);
    outcome.process_s = mono_s + stereo_s;
    Ok(outcome)
}

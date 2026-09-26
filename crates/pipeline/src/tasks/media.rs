//! The media tasks: probe and decode, the shot scan, vocal separation.
//!
//! **Role:** stream the mix to 16 kHz, scan shot changes with their scores, and split the vocal
//! and background stems with the chosen separator.
//!
//! **Position:** called by `tasks::run` inside workers of the main binary (FFmpeg as their child;
//! ONNX Runtime for separation).
//!
//! **Signals and state:** reads the video; writes `probe.json`, `audio/mix_16k.f32`, `shots.json`
//! and both stems.
//!
//! **Invariants:** the video is only read; audio is streamed, never held whole.

use std::time::{Duration, Instant};

use inference::onnx::separation::{MdxNet, MelRoformer, mdx_net};
use job_model::job::Separator;
use job_model::outputs::ProbeDecoded;
use media_io::{Programs, shot_changes};
use stages::separation::{self, SeparationRequest};

use super::{Job, TaskReport, since};
use crate::error::{Context, Result};
use crate::work_dir;

/// The separation models, as the model store names them.
const ROFORMER: (&str, &str) = (
    "mel-band-roformer-vocals",
    "syhft_core_folded_fp16_webgpu.onnx",
);
const MDX_NET: (&str, &str) = ("mdx-net-voc-ft", "UVR-MDX-NET-Voc_FT.onnx");
/// The longest a shot scan or a separation may keep FFmpeg running.
const MEDIA_DEADLINE: Duration = Duration::from_secs(3 * 3600);

pub(super) fn probe_decode(job: &Job) -> Result<TaskReport> {
    audio_folder(job)?;
    let started = Instant::now();
    let decoded = stages::probe_decode::probe_and_decode(
        &Programs::default(),
        &job.video(),
        job.settings().audio_track,
        &job.work.mix(),
    )
    .context("probe and decode")?;
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("audio_track", decoded.track.audio_position);
    report.note("duration_s", format!("{:.1}", decoded.probe.duration_s));
    if let Some(fps) = decoded.probe.video.as_ref().and_then(|v| v.fps()) {
        report.note("fps", format!("{fps:.3}"));
    }
    let output = ProbeDecoded {
        probe: decoded.probe,
        track: decoded.track,
        samples: decoded.samples,
    };
    work_dir::write_json(&job.work.probe(), &output)?;
    Ok(report)
}

pub(super) fn shot_scan(job: &Job) -> Result<TaskReport> {
    let started = Instant::now();
    let shots = shot_changes::scan(&Programs::default(), &job.video(), false, MEDIA_DEADLINE)
        .context("shot scan")?;
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    report.note("changes", shots.cuts.len());
    report.note(
        "cuts_at_cut_score",
        shots.times_at_least(job.settings().cut_score).len(),
    );
    work_dir::write_json(&job.work.shots(), &shots)?;
    Ok(report)
}

pub(super) fn separation(job: &Job) -> Result<TaskReport> {
    audio_folder(job)?;
    let probe = job.probe()?;
    let models = job.models()?;
    let programs = Programs::default();
    let (vocals, background) = (job.work.vocals(), job.work.background());
    let request = SeparationRequest {
        programs: &programs,
        video: &job.video(),
        audio_position: probe.track.audio_position,
        deadline: MEDIA_DEADLINE,
        vocals_16k: &vocals,
        background_16k: &background,
    };
    let load = Instant::now();
    let mut report = TaskReport::default();
    let summary = match job.settings().separator {
        Separator::Roformer => {
            let model = MelRoformer::open(&models.join(ROFORMER.0).join(ROFORMER.1))
                .context("load Mel-Band RoFormer")?;
            report.load_s = since(load);
            separation::separate(model, &request).context("separate")?.0
        }
        Separator::MdxNet => {
            let model = MdxNet::open(&models.join(MDX_NET.0).join(MDX_NET.1), mdx_net::VOC_FT, 1)
                .context("load MDX-Net")?;
            report.load_s = since(load);
            separation::separate(model, &request).context("separate")?.0
        }
    };
    report.process_s = since(load) - report.load_s;
    report.note("decode_s", format!("{:.1}", summary.decode_s));
    report.note("samples_16k", summary.samples_16k);
    Ok(report)
}

/// The folder the audio files are streamed into.
fn audio_folder(job: &Job) -> Result<()> {
    let folder = job.work.root().join("audio");
    std::fs::create_dir_all(&folder).context(format!("cannot create {}", folder.display()))
}

//! The media tasks: probe and decode, the shot scan, vocal separation.
//!
//! **Role:** stream the mix to 16 kHz, scan shot changes with their scores, and split the vocal
//! and background stems with the chosen separator.
//!
//! **Position:** called by `tasks::run` inside workers of the main binary (FFmpeg as their child;
//! ONNX Runtime for separation).
//!
//! **Signals and state:** reads the video and the probe; stores the probe (`outputs/probe_decode`)
//! and the shot changes (`outputs/shot_scan`); streams `audio/mix_16k.f32` and both stems.
//!
//! **Invariants:** the video is only read; audio is streamed, never held whole; the separation's
//! stems are files its step record names, with no document of their own.

use std::time::{Duration, Instant};

use inference::onnx::separation::{MdxNet, MelRoformer, mdx_net};
use job_model::StepName;
use job_model::job::Separator;
use job_model::outputs::ProbeDecoded;
use media_io::{Programs, shot_changes};
use stages::separation::{self, SeparationRequest};

use super::{Job, StepIo, StepProgress, TaskReport, since};
use crate::error::{Context, Result};
use crate::models;
use crate::work_dir;

/// The longest a shot scan or a separation may keep FFmpeg running.
const MEDIA_DEADLINE: Duration = Duration::from_secs(3 * 3600);

pub(super) fn probe_decode(
    job: &Job,
    io: &mut StepIo,
    progress: StepProgress,
) -> Result<TaskReport> {
    audio_folder(job)?;
    let started = Instant::now();
    let decoded = stages::probe_decode::probe_and_decode(
        &Programs::beside_current_exe(),
        &job.video(),
        job.settings().audio_track,
        &job.work.mix(),
        progress,
    )
    .context("probe and decode")?;
    work_dir::sync_file(&job.work.mix())?;
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
    io.put(StepName::ProbeDecode, None, &output)?;
    Ok(report)
}

pub(super) fn shot_scan(job: &Job, io: &mut StepIo, _progress: StepProgress) -> Result<TaskReport> {
    let started = Instant::now();
    let shots = shot_changes::scan(
        &Programs::beside_current_exe(),
        &job.video(),
        false,
        MEDIA_DEADLINE,
    )
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
    io.put(StepName::ShotScan, None, &shots)?;
    Ok(report)
}

pub(super) fn separation(job: &Job, io: &mut StepIo, progress: StepProgress) -> Result<TaskReport> {
    audio_folder(job)?;
    let probe = io.probe()?;
    let root = job.models()?;
    let programs = Programs::beside_current_exe();
    let (vocals, background) = (job.work.vocals(), job.work.background());
    let request = SeparationRequest {
        programs: &programs,
        video: &job.video(),
        audio_position: probe.track.audio_position,
        deadline: MEDIA_DEADLINE,
        vocals_16k: &vocals,
        background_16k: &background,
        duration_s: probe.probe.duration_s,
        progress,
    };
    let load = Instant::now();
    let mut report = TaskReport::default();
    let summary = match job.settings().separator {
        Separator::Roformer => {
            let (folder, file) = models::separator_model(Separator::Roformer);
            let model = MelRoformer::open(&root.join(folder).join(file))
                .context("load Mel-Band RoFormer")?;
            report.load_s = since(load);
            separation::separate(model, &request).context("separate")?.0
        }
        Separator::MdxNet => {
            let (folder, file) = models::separator_model(Separator::MdxNet);
            let model = MdxNet::open(&root.join(folder).join(file), mdx_net::VOC_FT, 1)
                .context("load MDX-Net")?;
            report.load_s = since(load);
            separation::separate(model, &request).context("separate")?.0
        }
    };
    work_dir::sync_file(&vocals)?;
    work_dir::sync_file(&background)?;
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

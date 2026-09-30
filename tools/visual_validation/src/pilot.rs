//! Measured visual pilots using the production step graph and worker isolation.
//!
//! **Role:** exercise all visual steps without repeating speech inference during validation.
//! **Position:** validation command above pipeline tasks, resume and workers.
//! **Signals and state:** a dedicated work directory, six stage artifacts and a preview ASS.
//! **Invariants:** source media and its installed subtitles remain untouched; measurements are real.

use anyhow::{Context, Result, ensure};
use inference::{cuda_runtime::CudaRuntime, model_store};
use job_model::{
    StepName,
    job::{JobRecord, JobSettings, StepRecord},
    outputs::ProbeDecoded,
};
use pipeline::{
    cancel::CancelToken,
    graph::{self, Placement},
    progress::Progress,
    resume,
    tasks::{self, Job},
    work_dir::{self, JobStore, WorkDir},
    workers::{self, Binaries},
};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use subtitle_formats::{
    cue::{CueTrack, FrameRate},
    writers::ass,
};

pub fn run(
    video: &Path,
    output: &Path,
    binaries: &Path,
    dialogue: Option<&Path>,
    claude: bool,
) -> Result<()> {
    let video = video.canonicalize()?;
    let binaries = binaries.canonicalize()?;
    let work = WorkDir::new(output);
    let _store = JobStore::open(&work)?;
    let meta = std::fs::metadata(&video)?;
    ensure!(meta.is_file(), "pilot input must be a video file");
    let modified_s = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let glossary = stages::adjudication::glossary::one_piece();
    let mut record = if work.job_json().exists() {
        let record: JobRecord = work_dir::read_json(&work.job_json())?;
        ensure!(
            Path::new(&record.video) == video,
            "work directory belongs to another video"
        );
        ensure!(
            record.video_size == meta.len() && record.video_modified_s == modified_s,
            "pilot video size or modification time changed; choose a new output directory so earlier observations are not reused"
        );
        record
    } else {
        JobRecord {
            video: video.to_string_lossy().into_owned(),
            video_size: meta.len(),
            video_modified_s: modified_s,
            settings: JobSettings::with_glossary(glossary.clone()),
            models_dir: None,
            corrections: None,
            steps: Default::default(),
        }
    };
    record.settings.glossary = glossary;
    record.settings.onscreen_text.enabled = true;
    record.settings.onscreen_text.claude_fallback = claude;
    let programs = media_io::Programs::beside_current_exe();
    if !work.probe().exists() {
        let probe = media_io::probe::probe(&programs, &video)?;
        let track = media_io::probe::english_track(&probe)?.clone();
        work_dir::write_json(
            &work.probe(),
            &ProbeDecoded {
                probe,
                track,
                samples: 0,
            },
        )?;
    }
    let probe: ProbeDecoded = work_dir::read_json(&work.probe())?;
    let stream = probe.probe.video.as_ref().context("pilot has no video")?;
    let frame_rate = FrameRate::new(stream.frame_rate_num, stream.frame_rate_den)
        .context("invalid frame rate")?;
    let cues: CueTrack = if let Some(path) = dialogue {
        work_dir::read_json(path)?
    } else {
        CueTrack {
            frame_rate,
            cues: Vec::new(),
        }
    };
    let same_cues =
        work_dir::read_json::<CueTrack>(&work.cues()).is_ok_and(|previous| previous == cues);
    if !same_cues || !record.steps.contains_key(&StepName::Cues) {
        if !same_cues {
            work_dir::write_json(&work.cues(), &cues)?;
        }
        record.steps.insert(
            StepName::Cues,
            StepRecord {
                fingerprint: "validation dialogue input".into(),
                finished_ns: now(),
                measure: Default::default(),
            },
        );
    }
    if !work.shots().exists() {
        let shots =
            media_io::shot_changes::scan(&programs, &video, false, Duration::from_secs(3600))?;
        work_dir::write_json(&work.shots(), &shots)?;
        record.steps.insert(
            StepName::ShotScan,
            StepRecord {
                fingerprint: "validation shot scan".into(),
                finished_ns: now(),
                measure: Default::default(),
            },
        );
    }
    work_dir::write_json(&work.job_json(), &record)?;
    let paths = Binaries {
        main: binaries.join("tbd-subtitles"),
        ggml: binaries.join("tbd-subtitles-ggml"),
        local_llm: binaries.join("tbd-subtitles-llm"),
    };
    let runtime = CudaRuntime::locate(Some(&binaries), &model_store::runtime_dir()?)?;
    let cancel = CancelToken::default();
    let lock = model_store::app_data_dir()?.join("gpu.lock");
    let steps = [
        StepName::TextDetect,
        StepName::TextRead,
        StepName::TextTrack,
        StepName::TextTranslate,
        StepName::TextReview,
        StepName::TextTypeset,
    ];
    for step in steps {
        if resume::is_valid(step, &record, &work) {
            eprintln!("{step}: resumed");
            continue;
        }
        let fingerprint = resume::fingerprint_in_work(step, &record, &work);
        eprintln!("{step}: running");
        let progress = |event| match event {
            Progress::StepAdvanced { done, total, .. } if done == total || done % 240 == 0 => {
                eprintln!("{step}: {done}/{total}")
            }
            Progress::StepMessage { text, .. } => eprintln!("{step}: {text}"),
            Progress::ModelCall { call, .. } => {
                let _ = work_dir::write_json(
                    &work
                        .root()
                        .join(format!("visual/model-call-{}.json", now())),
                    &call,
                );
            }
            _ => {}
        };
        let measure = match graph::placement(step) {
            Placement::Worker(binary) => workers::run_worker(
                paths.path(binary),
                step,
                &work,
                &runtime.worker_env(),
                &progress,
                &cancel,
                &lock,
            )?,
            Placement::InProcess => tasks::in_process(
                step,
                &Job {
                    work: work.clone(),
                    record: record.clone(),
                },
                &|_, _| {},
            )?,
        };
        eprintln!(
            "{step}: {:.2}s, RAM {:?} MiB, VRAM {:?} MiB",
            measure.wall_s, measure.peak_ram_mib, measure.peak_vram_mib
        );
        record.steps.insert(
            step,
            StepRecord {
                fingerprint,
                finished_ns: now(),
                measure,
            },
        );
        work_dir::write_json(&work.job_json(), &record)?;
    }
    let mut rendered = ass::write(&cues);
    rendered.push_str(&std::fs::read_to_string(work.text_ass())?);
    work_dir::write_text(&work.root().join("preview.ass"), &rendered)?;
    let duration = probe.probe.duration_s;
    let elapsed: f64 = record
        .steps
        .iter()
        .filter(|(step, _)| steps.contains(step))
        .map(|(_, r)| r.measure.wall_s)
        .sum();
    println!(
        "Visual processing: {elapsed:.2}s for {duration:.2}s of source; {}",
        work.root().join("preview.ass").display()
    );
    Ok(())
}

fn now() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

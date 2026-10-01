//! Measured visual pilots using the production step graph and worker isolation.
//!
//! **Role:** exercise all visual steps without repeating speech inference during validation.
//! **Position:** validation command above pipeline tasks, resume and workers.
//! **Signals and state:** a dedicated work directory and its job database, six stage outputs and
//! a preview ASS.
//! **Invariants:** source media and its installed subtitles remain untouched; measurements are real.

use anyhow::{Context, Result, ensure};
use inference::{cuda_runtime::CudaRuntime, model_store};
use job_model::{
    StepName,
    job::{JobRecord, JobSettings, StepRecord},
    outputs::{ProbeDecoded, ShotChanges},
};
use pipeline::{
    cancel::CancelToken,
    graph::{self, Placement},
    progress::Progress,
    resume, runner,
    tasks::{self, Job, StepIo},
    work_dir::{self, JobStore, WorkDir},
    workers::{self, Binaries, StepWrite, WorkerData},
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
    let store = JobStore::open(&work)?;
    let meta = std::fs::metadata(&video)?;
    ensure!(meta.is_file(), "pilot input must be a video file");
    let modified_s = meta.modified()?.duration_since(UNIX_EPOCH)?.as_secs() as i64;
    let glossary = stages::adjudication::glossary::one_piece();
    let mut record = if let Some(record) = work_dir::load_job_record(&store)? {
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
        }
    };
    record.settings.glossary = glossary;
    record.settings.onscreen_text.enabled = true;
    record.settings.onscreen_text.claude_fallback = claude;
    let programs = media_io::Programs::beside_current_exe();
    let stored_probe = |store: &JobStore| -> Result<Option<ProbeDecoded>> {
        Ok(store.read()?.output(StepName::ProbeDecode, None)?)
    };
    if stored_probe(&store)?.is_none() {
        let probe = media_io::probe::probe(&programs, &video)?;
        let track = media_io::probe::english_track(&probe)?.clone();
        store.put_output(
            StepName::ProbeDecode,
            None,
            &ProbeDecoded {
                probe,
                track,
                samples: 0,
            },
        )?;
    }
    let probe = stored_probe(&store)?.context("the probe is stored")?;
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
    let same_cues = store
        .read()?
        .output::<CueTrack>(StepName::Cues, None)
        .ok()
        .flatten()
        .is_some_and(|previous| previous == cues);
    store.put_job_record(&record)?;
    let finished = |fingerprint: &str| StepRecord {
        fingerprint: fingerprint.into(),
        finished_ns: now(),
        measure: Default::default(),
    };
    if !same_cues || store.read()?.step_record(StepName::Cues)?.is_none() {
        if !same_cues {
            store.put_output(StepName::Cues, None, &cues)?;
            let dropped: Vec<String> = Vec::new();
            store.put_output(
                StepName::Cues,
                Some(work_dir::store::keys::DROPPED_SOUNDS),
                &dropped,
            )?;
        }
        store.put_step_record(StepName::Cues, &finished("validation dialogue input"))?;
    }
    let shots_stored = store
        .read()?
        .output::<ShotChanges>(StepName::ShotScan, None)?
        .is_some();
    if !shots_stored {
        let shots =
            media_io::shot_changes::scan(&programs, &video, false, Duration::from_secs(3600))?;
        store.put_output(StepName::ShotScan, None, &shots)?;
        store.put_step_record(StepName::ShotScan, &finished("validation shot scan"))?;
    }
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
        let read = store.read()?;
        if resume::is_valid(step, &record, &read, &work) {
            eprintln!("{step}: resumed");
            continue;
        }
        let fingerprint = resume::fingerprint(step, &record, &read)?;
        let inputs = runner::worker_inputs(step, &read)?;
        drop(read);
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
        let (measure, outputs) = match graph::placement(step) {
            Placement::Worker(binary) => {
                let data = WorkerData {
                    store: &store,
                    inputs: &inputs,
                };
                let run = workers::run_worker(
                    paths.path(binary),
                    step,
                    data,
                    &runtime.worker_env(),
                    &progress,
                    &cancel,
                    &lock,
                )?;
                (run.measure, run.outputs)
            }
            Placement::InProcess => {
                let mut io = StepIo::in_process(&store)?;
                let job = Job {
                    work: work.clone(),
                    record: record.clone(),
                };
                let measure = tasks::in_process(step, &job, &mut io, &|_, _| {})?;
                (measure, io.into_outputs())
            }
        };
        eprintln!(
            "{step}: {:.2}s, RAM {:?} MiB, VRAM {:?} MiB",
            measure.wall_s, measure.peak_ram_mib, measure.peak_vram_mib
        );
        let stamped = StepRecord {
            fingerprint,
            finished_ns: now(),
            measure,
        };
        outputs
            .unwrap_or_else(|| StepWrite::new(store.clone()))
            .commit(step, &stamped)?;
    }
    let mut rendered = ass::write(&cues);
    let events: String = store
        .read()?
        .output(
            StepName::TextTypeset,
            Some(work_dir::store::keys::TYPESET_ASS),
        )?
        .context("the typeset ASS events are stored")?;
    rendered.push_str(&events);
    work_dir::write_text(&work.root().join("preview.ass"), &rendered)?;
    let duration = probe.probe.duration_s;
    let elapsed: f64 = work_dir::load_step_records(&store)?
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

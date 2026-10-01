//! The Check Text preview against a real finished pilot job, and the pilot layout it reads.

use super::*;

/// Exercise real preview decoding only; this does not validate OCR, translation or VLC playback.
#[test]
#[ignore = "host FFmpeg/GPU/audio; set TBD_M4_PREVIEW_WORK and TBD_M4_PREVIEW_VIDEO"]
fn real_visual_preview_host() {
    let source_work = WorkDir::new(
        PathBuf::from(std::env::var_os("TBD_M4_PREVIEW_WORK").expect("finished pilot work folder"))
            .canonicalize()
            .expect("existing pilot work folder"),
    );
    let source_video =
        PathBuf::from(std::env::var_os("TBD_M4_PREVIEW_VIDEO").expect("pilot video clip"))
            .canonicalize()
            .expect("existing pilot video clip");
    let video_before = fs::metadata(&source_video).expect("source video metadata");
    let (source_ass_path, source_ass) =
        pilot_ass(&source_work).expect("actual exported or pilot ASS");
    let source_document = typeset_bytes(&source_work);
    let mut fixture = Fixture::new("real-preview");
    install_preview_pilot(&mut fixture, &source_work, &source_video, &source_ass);
    let snapshots = std::env::var_os("TBD_SNAPSHOTS").map(PathBuf::from);
    let mut harness = fixture.harness_with(
        snapshots.is_some(),
        Arc::new(|_, _, _| panic!("a read-only preview test must never run the pipeline")),
    );
    open_text(&mut harness);
    let (selected, fps, initial_time) = {
        let session = harness.state().text.session.as_ref().expect("loaded pilot");
        assert_eq!(session.video, source_video);
        assert_eq!(session.ass, fixture.root.join("pilot.ass"));
        let requested = std::env::var("TBD_M4_PREVIEW_TEXT").ok();
        let (index, occurrence) = session
            .document
            .occurrences
            .iter()
            .enumerate()
            .filter(|(_, text)| {
                text.rendered == Some(true)
                    && text
                        .english
                        .as_ref()
                        .is_some_and(|text| !text.trim().is_empty())
                    && text.end_s - text.start_s >= 0.8
                    && requested.as_ref().is_none_or(|id| id == &text.id)
            })
            .max_by(|(_, a), (_, b)| (a.end_s - a.start_s).total_cmp(&(b.end_s - b.start_s)))
            .expect("pilot needs a rendered English occurrence lasting at least 0.8 seconds");
        assert!(session.thumbnails[index].is_some(), "pilot thumbnail loads");
        let time = occurrence.start_s + (2.0 / session.fps).max(0.1);
        eprintln!(
            "Real preview: {} at {time:.3}s, {:.3} fps, exported ASS {}",
            occurrence.id,
            session.fps,
            source_ass_path.display()
        );
        (index, session.fps, time)
    };
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Select(selected));
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Seek(initial_time));
    let first = wait_real_comparison(&mut harness, initial_time);
    assert_preview_pixels(&first, true);

    harness.get_by_label("Next frame").click();
    harness.run_steps(2);
    let next_time = preview_position(&harness);
    assert!(next_time > initial_time);
    assert!(next_time - initial_time <= 2.0 / fps + 0.001);
    let next = wait_real_comparison(&mut harness, next_time);
    assert_ne!(next.original.serial, first.original.serial);
    assert_preview_pixels(&next, false);
    harness.get_by_label("Previous frame").click();
    harness.run_steps(2);
    let previous_time = preview_position(&harness);
    assert!(previous_time < next_time);
    let _ = wait_real_comparison(&mut harness, previous_time);

    harness.get_by_label("Preview position").focus();
    harness.run_steps(1);
    harness.key_press(egui::Key::ArrowRight);
    harness.run_steps(2);
    let scrubbed_time = preview_position(&harness);
    assert!(
        scrubbed_time > previous_time,
        "scrubbing moves the requested time"
    );
    let _ = wait_real_comparison(&mut harness, scrubbed_time);
    harness.get_by_label("Play").click();
    wait_real_preview(&mut harness, |app| {
        app.text.player.as_ref().is_some_and(|player| {
            player.is_playing()
                && player
                    .comparison()
                    .is_some_and(|pair| pair.time_s >= scrubbed_time + 0.25)
        })
    });
    let playing = preview_comparison(harness.state()).expect("playing comparison");
    assert_preview_pixels(&playing, false);
    harness.get_by_label("Stop").click();
    wait_real_preview(&mut harness, |app| {
        app.text
            .player
            .as_ref()
            .is_some_and(|player| !player.is_playing())
    });

    // Capture a still with visible exported text instead of a possibly transitional playback frame.
    harness
        .state_mut()
        .apply_text(crate::text_review::models::Event::Seek(initial_time));
    let displayed = wait_real_comparison(&mut harness, initial_time);
    assert_preview_pixels(&displayed, true);
    if let Some(folder) = snapshots {
        fs::create_dir_all(&folder).expect("real preview screenshot folder");
        for (scheme, name) in [(Scheme::Light, "light"), (Scheme::Dark, "dark")] {
            harness.state_mut().scheme = scheme;
            harness.run_steps(3);
            harness
                .render()
                .expect("render real Check Text preview")
                .save(folder.join(format!("m4-real-check-text-{name}.png")))
                .expect("save real Check Text screenshot");
        }
    }
    harness.get_by_label("Overview").click();
    harness.run_steps(2);
    assert!(harness.state().text.player.is_none());
    assert!(harness.state().text.session.is_none());
    assert!(fixture.calls.lock().expect("calls").is_empty());
    let video_after = fs::metadata(&source_video).expect("source video remains");
    assert_eq!(video_after.len(), video_before.len());
    assert_eq!(
        video_after.modified().unwrap(),
        video_before.modified().unwrap()
    );
    assert_eq!(fs::read(&source_ass_path).unwrap(), source_ass);
    assert_eq!(typeset_bytes(&source_work), source_document);
}

/// The bytes of the typeset text the job in `work` stores.
fn typeset_bytes(work: &WorkDir) -> Vec<u8> {
    let key = work_dir::store::keys::output_key(StepName::TextTypeset, None);
    work_dir::read_stored(work.root(), |read| {
        read.raw(worker_channel::address::Table::Outputs, &key)
    })
    .expect("the pilot's database")
    .flatten()
    .expect("the pilot's typeset text")
}

fn pilot_ass(source: &WorkDir) -> Result<(PathBuf, Vec<u8>), String> {
    let installed = work_dir::read_stored(source.root(), |read| {
        read.output::<OutputRecord>(StepName::Output, None)
    })
    .ok()
    .flatten()
    .flatten()
    .map(|record| PathBuf::from(record.path));
    installed
        .into_iter()
        .chain([source.root().join("preview.ass")])
        .find_map(|path| {
            let bytes = fs::read(&path).ok()?;
            let text = std::str::from_utf8(&bytes).ok()?;
            (text.contains("[Script Info]") && text.contains("[Events]")).then_some((path, bytes))
        })
        .ok_or_else(|| {
            format!(
                "No valid exported ASS or visual_validation preview.ass in {}",
                source.root().display()
            )
        })
}

#[test]
fn pilot_fixture_accepts_the_validation_layout_and_refuses_missing_or_invalid_ass() {
    let fixture = Fixture::new("pilot-layout");
    let installed = fixture.video.with_extension("ass");
    let expected = fs::read(&installed).unwrap();
    assert_eq!(pilot_ass(&fixture.work).unwrap().0, installed);
    remove_output(&fixture.work);
    assert!(pilot_ass(&fixture.work).is_err());
    let preview = fixture.work.root().join("preview.ass");
    fs::write(&preview, b"incomplete output").unwrap();
    assert!(pilot_ass(&fixture.work).is_err());
    fs::write(&preview, &expected).unwrap();
    assert_eq!(
        pilot_ass(&fixture.work).unwrap(),
        (preview.clone(), expected)
    );
    assert_eq!(
        work_dir::read_stored(fixture.work.root(), |read| {
            read.output::<OutputRecord>(StepName::Output, None)
        })
        .expect("read"),
        Some(None),
        "the pilot is read-only"
    );
    fs::remove_file(preview).unwrap();
    assert!(pilot_ass(&fixture.work).is_err());
}

/// Take the output record out of the database of the job in `work`.
fn remove_output(work: &WorkDir) {
    let store = store(work);
    let mut write = store.write().expect("write");
    write
        .remove(
            worker_channel::address::Table::Outputs,
            &work_dir::store::keys::output_key(StepName::Output, None),
        )
        .expect("remove");
    write.commit().expect("commit");
}

fn install_preview_pilot(fixture: &mut Fixture, source: &WorkDir, video: &Path, ass: &[u8]) {
    let (job, probe, document, qc) = work_dir::read_stored(source.root(), |read| {
        Ok((
            read.job_record()?,
            read.output::<ProbeDecoded>(StepName::ProbeDecode, None)?,
            read.output::<TextDocument>(StepName::TextTypeset, None)?,
            read.output::<QcReport>(StepName::Qc, None)?,
        ))
    })
    .expect("the pilot's database")
    .expect("a pilot database");
    let mut job = job.expect("the pilot's job record");
    assert_eq!(
        Path::new(&job.video)
            .canonicalize()
            .expect("recorded pilot video"),
        video,
        "work folder and supplied video must describe the same source timeline"
    );
    let probe = probe.expect("the pilot's probe");
    let mut document = document.expect("the pilot's typeset text");
    assert!(
        document.occurrences.len() <= 1000,
        "use a bounded pilot clip"
    );
    fixture.video = video.into();
    fixture.work = WorkDir::new(fixture.root.join("work").join(work_dir::job_id(video)));
    job.video = video.to_string_lossy().into_owned();
    // The comparison under test is the source against its exported ASS.
    job.settings.onscreen_text.localized_video = false;
    let store = store(&fixture.work);
    store.put_job_record(&job).expect("job record");
    store
        .put_output(StepName::ProbeDecode, None, &probe)
        .expect("probe");
    let mut copied_bytes = 0;
    for (index, text) in document.occurrences.iter_mut().enumerate() {
        let crop = text.crops.iter().find_map(|relative| {
            if relative.components().any(|part| {
                !matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            }) {
                return None;
            }
            let crop = source.root().join(relative).canonicalize().ok()?;
            crop.starts_with(source.root()).then_some(crop)
        });
        text.crops.clear();
        if let Some(crop) = crop {
            let bytes = fs::metadata(&crop).expect("pilot crop metadata").len();
            copied_bytes += bytes;
            assert!(
                copied_bytes <= 64 * 1024 * 1024,
                "pilot crop copy exceeds 64 MiB"
            );
            let relative = PathBuf::from(format!("visual/pilot-{index}.png"));
            let destination = fixture.work.root().join(&relative);
            fs::create_dir_all(destination.parent().unwrap()).expect("scratch crop folder");
            fs::copy(crop, destination).expect("copy read-only pilot crop");
            text.crops.push(relative);
        }
    }
    store
        .put_output(StepName::TextTypeset, None, &document)
        .expect("typeset");
    let mut qc = qc.unwrap_or_default();
    qc.summary.video_s = probe.probe.duration_s;
    store.put_output(StepName::Qc, None, &qc).expect("qc");
    let ass_path = fixture.root.join("pilot.ass");
    fs::write(&ass_path, ass).expect("scratch copy of exported ASS");
    let output = OutputRecord {
        path: ass_path.to_string_lossy().into_owned(),
        ..Default::default()
    };
    store
        .put_output(StepName::Output, None, &output)
        .expect("output");
}

fn preview_position(harness: &Harness<'_, TbdSubtitlesApp>) -> f64 {
    harness
        .state()
        .text
        .session
        .as_ref()
        .expect("pilot session")
        .position_s
}

fn preview_comparison(app: &TbdSubtitlesApp) -> Option<crate::text_review::models::Comparison> {
    app.text
        .player
        .as_ref()
        .and_then(|player| player.comparison())
}

fn wait_real_preview(
    harness: &mut Harness<'_, TbdSubtitlesApp>,
    ready: impl Fn(&TbdSubtitlesApp) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        harness.run_steps(1);
        let app = harness.state();
        assert!(
            app.text.error.is_none(),
            "preview load error: {:?}",
            app.text.error
        );
        if let Some(player) = &app.text.player {
            assert!(
                player.error().is_none(),
                "FFmpeg preview failed: {:?}",
                player.error()
            );
        }
        if ready(app) {
            harness.run_steps(2);
            return;
        }
        assert!(Instant::now() < deadline, "real FFmpeg preview timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_real_comparison(
    harness: &mut Harness<'_, TbdSubtitlesApp>,
    time: f64,
) -> crate::text_review::models::Comparison {
    wait_real_preview(harness, |app| {
        preview_comparison(app).is_some_and(|pair| (pair.time_s - time).abs() < 0.001)
    });
    preview_comparison(harness.state()).expect("ready comparison")
}

fn assert_preview_pixels(pair: &crate::text_review::models::Comparison, translated: bool) {
    let rendered = pair.rendered.as_ref().expect("the subtitles picture");
    for picture in [&pair.original, rendered] {
        assert!(picture.width > 0 && picture.height > 0);
        assert!(picture.width <= 720 && picture.height <= 720);
        assert_eq!(
            picture.rgb.len(),
            (picture.width * picture.height * 3) as usize
        );
        assert!(
            picture.rgb.iter().filter(|&&value| value > 16).count() > picture.rgb.len() / 100,
            "preview must contain an actual visible scene, not a black/empty frame"
        );
    }
    assert_eq!(
        (pair.original.width, pair.original.height),
        (rendered.width, rendered.height)
    );
    if translated {
        let changed = pair
            .original
            .rgb
            .iter()
            .zip(&rendered.rgb)
            .filter(|(left, right)| left.abs_diff(**right) > 8)
            .count();
        assert!(
            changed > 100,
            "the exported ASS must visibly change the rendered comparison"
        );
    }
}

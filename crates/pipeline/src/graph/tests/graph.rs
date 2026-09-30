use super::*;

fn position(step: StepName) -> usize {
    StepName::ALL
        .iter()
        .position(|s| *s == step)
        .expect("listed")
}

#[test]
fn every_step_reads_only_earlier_steps() {
    for step in StepName::ALL {
        for input in inputs(step) {
            assert!(
                position(*input) < position(step),
                "{step} reads the later {input}"
            );
        }
    }
}

#[test]
fn alignment_and_review_read_both_transcripts() {
    use StepName::*;
    assert_eq!(
        inputs(Alignment),
        &[
            ProbeDecode,
            Separation,
            AsrParakeet,
            AsrWhisper,
            DiffSheet,
            Readjudicate
        ]
    );
    assert_eq!(
        inputs(Review),
        &[
            ProbeDecode,
            Separation,
            AsrParakeet,
            AsrWhisper,
            DiffSheet,
            Readjudicate,
            Alignment
        ]
    );
}

#[test]
fn changed_steps_carry_their_revision_and_the_rest_are_at_one() {
    use StepName::*;
    assert_eq!(revision(Alignment), 2);
    assert_eq!(revision(Review), 2);
    assert_eq!(revision(Cues), 3);
    assert_eq!(revision(Qc), 5);
    assert_eq!(revision(Output), 5);
    assert_eq!(revision(TextDetect), 4);
    assert_eq!(revision(TextRead), 3);
    assert_eq!(revision(TextTrack), 3);
    assert_eq!(revision(TextTranslate), 7);
    assert_eq!(revision(TextReview), 3);
    assert_eq!(revision(TextTypeset), 4);
    assert_eq!(revision(TextMask), 2);
    assert_eq!(revision(TextInpaint), 3);
    assert_eq!(revision(TextCompose), 2);
    assert_eq!(revision(TextVerify), 1);
    assert_eq!(revision(LocalizedVideo), 2);
    for step in StepName::ALL {
        if !matches!(
            step,
            Alignment
                | Review
                | Cues
                | Qc
                | Output
                | TextDetect
                | TextRead
                | TextTrack
                | TextTranslate
                | TextReview
                | TextTypeset
                | TextMask
                | TextInpaint
                | TextCompose
                | LocalizedVideo
        ) {
            assert_eq!(revision(step), 1, "{step}");
        }
    }
}

#[test]
fn gpu_steps_run_in_workers_and_whisper_alone_in_the_ggml_binary() {
    for step in StepName::ALL {
        if uses_gpu(step) {
            assert!(matches!(placement(step), Placement::Worker(_)), "{step}");
        }
        let ggml = matches!(step, StepName::AsrWhisper | StepName::RedecodeWhisper);
        assert_eq!(
            placement(step) == Placement::Worker(Binary::Ggml),
            ggml,
            "{step}"
        );
    }
}

#[test]
fn visual_typesetting_runs_in_a_cancellable_worker_without_a_gpu_runtime_or_lock() {
    assert_eq!(
        placement(StepName::TextTypeset),
        Placement::Worker(Binary::Main)
    );
    assert!(!uses_gpu(StepName::TextTypeset));
    assert!(!loads_onnx_runtime(StepName::TextTypeset));
}

#[test]
fn whisper_and_onnx_runtime_workers_get_the_cuda_runtime() {
    use StepName::*;
    for step in [AsrWhisper, RedecodeWhisper] {
        assert!(
            needs_cuda_runtime(step) && !loads_onnx_runtime(step),
            "{step}"
        );
    }
    assert!(needs_cuda_runtime(AsrParakeet));
    for step in [TextTranslate, TextMask, LocalizedVideo, Vad] {
        assert!(!needs_cuda_runtime(step), "{step}");
    }
}

#[test]
fn replacement_steps_keep_onnx_runtime_and_the_gpu_lock_to_inpainting_and_encoding() {
    use StepName::*;
    for step in [TextMask, TextInpaint, TextCompose, LocalizedVideo] {
        assert_eq!(placement(step), Placement::Worker(Binary::Main), "{step}");
    }
    assert!(uses_gpu(TextInpaint) && loads_onnx_runtime(TextInpaint));
    assert!(uses_gpu(LocalizedVideo) && !loads_onnx_runtime(LocalizedVideo));
    for step in [TextMask, TextCompose] {
        assert!(!uses_gpu(step) && !loads_onnx_runtime(step), "{step}");
    }
}

#[test]
fn the_localized_video_switch_leaves_the_translation_fingerprint_alone() {
    let mut on = JobSettings::with_glossary(Vec::new());
    on.onscreen_text.enabled = true;
    on.onscreen_text.localized_video = false;
    let mut both = on.clone();
    both.onscreen_text.localized_video = true;
    assert_eq!(
        settings(StepName::TextTranslate, &on),
        settings(StepName::TextTranslate, &both)
    );
    for step in [StepName::TextMask, StepName::TextTypeset, StepName::Output] {
        assert_ne!(settings(step, &on), settings(step, &both), "{step}");
    }
}

#[test]
fn every_step_leaves_at_least_one_file() {
    let work = WorkDir::new("/work/job");
    for step in StepName::ALL {
        assert!(
            !outputs(step, &work, Path::new("/v/a.mp4"), OutputFormat::Srt).is_empty(),
            "{step}"
        );
    }
    assert!(
        outputs(
            StepName::Output,
            &work,
            Path::new("/v/a.mp4"),
            OutputFormat::Srt
        )
        .contains(&PathBuf::from("/v/a.srt"))
    );
    assert!(
        outputs(
            StepName::Output,
            &work,
            Path::new("/v/a.mp4"),
            OutputFormat::Ass
        )
        .contains(&PathBuf::from("/v/a.ass"))
    );
}

#[test]
fn only_the_settings_a_step_reads_reach_its_fingerprint() {
    let mut a = JobSettings::with_glossary(vec![]);
    a.onscreen_text.enabled = false;
    let mut b = a.clone();
    b.cut_score = 30.0;
    assert_eq!(
        settings(StepName::Alignment, &a),
        settings(StepName::Alignment, &b)
    );
    assert_ne!(settings(StepName::Cues, &a), settings(StepName::Cues, &b));
    let mut c = b.clone();
    c.output_format = OutputFormat::Vtt;
    assert_ne!(
        settings(StepName::Output, &b),
        settings(StepName::Output, &c)
    );
    assert_eq!(settings(StepName::Cues, &b), settings(StepName::Cues, &c));
    a.glossary.push("Luffy".into());
    assert_ne!(
        settings(StepName::Adjudicate, &a),
        settings(StepName::Adjudicate, &b)
    );
}

struct ArtifactsFixture(WorkDir);

impl ArtifactsFixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "tbd-graph-artifacts-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("visual/crops")).unwrap();
        Self(WorkDir::new(root))
    }

    fn document(&self, value: Value) {
        std::fs::write(
            self.0.text(StepName::TextDetect),
            serde_json::to_vec(&value).unwrap(),
        )
        .unwrap();
    }

    fn valid(&self) -> bool {
        artifacts_valid(
            StepName::TextDetect,
            &self.0,
            Path::new("/source/video.mkv"),
            OutputFormat::Ass,
        )
    }
}

impl Drop for ArtifactsFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.root());
    }
}

#[test]
fn missing_crop_invalidates_detection_until_its_producer_restores_it() {
    let fixture = ArtifactsFixture::new("missing-crop");
    let first = fixture.0.root().join("visual/crops/first.png");
    let second = fixture.0.root().join("visual/crops/second.png");
    fixture.document(json!({"occurrences": [
        {"crops": ["visual/crops/first.png"], "frames": [{"quad": [0, 1, 2, 3]}]},
        {"crops": ["visual/crops/second.png"]}
    ]}));
    std::fs::write(&first, b"first representative crop").unwrap();
    std::fs::write(&second, b"second representative crop").unwrap();
    assert!(fixture.valid());

    std::fs::remove_file(&second).unwrap();
    assert!(fixture.0.text(StepName::TextDetect).is_file());
    assert!(first.is_file());
    assert!(!fixture.valid());
    std::fs::write(&second, b"regenerated representative crop").unwrap();
    assert!(fixture.valid());
}

#[test]
fn invalid_crop_references_and_incomplete_artifacts_cannot_resume_detection() {
    let fixture = ArtifactsFixture::new("invalid-crop");
    let crop = fixture.0.root().join("visual/crops/empty.png");
    let good = fixture.0.root().join("visual/crops/good.png");
    std::fs::write(&crop, []).unwrap();
    std::fs::write(&good, b"existing representative crop").unwrap();
    for crops in [
        json!([]),
        json!(["visual/crops/empty.png"]),
        json!(["visual/crops"]),
        json!(["visual/crops/../crops/good.png"]),
        json!([good]),
    ] {
        fixture.document(json!({"occurrences": [{"crops": crops}]}));
        assert!(!fixture.valid());
    }
    std::fs::write(fixture.0.text(StepName::TextDetect), b"{\"occurrences\": [").unwrap();
    assert!(!fixture.valid());
    fixture.document(json!({}));
    assert!(!fixture.valid());
}

#[test]
fn empty_detection_and_unrelated_audio_outputs_need_no_crop_files() {
    let fixture = ArtifactsFixture::new("empty-or-audio");
    fixture.document(serde_json::to_value(job_model::onscreen::TextDocument::default()).unwrap());
    assert!(fixture.valid());
    std::fs::remove_file(fixture.0.text(StepName::TextDetect)).unwrap();
    assert!(!fixture.valid());
    std::fs::write(fixture.0.vad(), b"{}").unwrap();
    assert!(artifacts_valid(
        StepName::Vad,
        &fixture.0,
        Path::new("/source/video.mkv"),
        OutputFormat::Srt,
    ));
}

#[test]
fn a_missing_keyframe_still_invalidates_detection_while_older_records_need_none() {
    let fixture = ArtifactsFixture::new("keyframe");
    let crop = fixture.0.root().join("visual/crops/first.png");
    std::fs::write(&crop, b"representative crop").unwrap();
    fixture.document(json!({"occurrences": [{"crops": ["visual/crops/first.png"]}]}));
    assert!(fixture.valid(), "records without a keyframe stay valid");
    fixture.document(json!({"occurrences": [{
        "crops": ["visual/crops/first.png"],
        "keyframe": {"time_s": 1.5, "image": "visual/keyframes/frame-00000036.png"}
    }]}));
    assert!(!fixture.valid(), "a referenced keyframe still must exist");
    std::fs::create_dir_all(fixture.0.root().join("visual/keyframes")).unwrap();
    let still = fixture.0.root().join("visual/keyframes/frame-00000036.png");
    std::fs::write(&still, []).unwrap();
    assert!(!fixture.valid(), "an empty still is not a keyframe");
    std::fs::write(&still, b"whole-frame still").unwrap();
    assert!(fixture.valid());
}

#[test]
fn the_output_and_the_localized_video_read_the_checked_replacements() {
    use StepName::*;
    assert_eq!(inputs(TextReview), &[ShotScan, TextTranslate]);
    assert_eq!(inputs(TextTypeset), &[TextReview]);
    assert_eq!(inputs(Output), &[Cues, TextTypeset, TextVerify]);
    assert_eq!(inputs(LocalizedVideo), &[ProbeDecode, TextVerify, Output]);
    assert_eq!(inputs(TextVerify), &[ProbeDecode, TextReview, TextCompose]);
    let work = WorkDir::new("/work/job");
    assert_eq!(
        outputs(TextTypeset, &work, Path::new("/v/a.mp4"), OutputFormat::Ass),
        vec![work.text(TextTypeset), work.text_ass()],
        "the localized subtitle file has no events of its own"
    );
}

#[test]
fn the_read_back_check_runs_between_composition_and_the_outputs_in_an_ocr_worker() {
    use StepName::*;
    let position = |step| StepName::ALL.iter().position(|s| *s == step).unwrap();
    assert_eq!(position(TextVerify), position(TextCompose) + 1);
    assert!(position(TextVerify) < position(Output));
    assert_eq!(StepName::ALL.len(), 29);
    assert_eq!(placement(TextVerify), Placement::Worker(Binary::Main));
    assert!(uses_gpu(TextVerify) && loads_onnx_runtime(TextVerify));
    let work = WorkDir::new("/work/job");
    assert_eq!(
        outputs(TextVerify, &work, Path::new("/v/a.mp4"), OutputFormat::Ass),
        vec![work.text(TextVerify)]
    );
    assert!(work.text(TextVerify).ends_with("visual/text_verify.json"));
    let mut job = JobSettings::with_glossary(Vec::new());
    let before = settings(TextVerify, &job);
    job.onscreen_text.localized_video = !job.onscreen_text.localized_video;
    assert_ne!(before, settings(TextVerify, &job));
}

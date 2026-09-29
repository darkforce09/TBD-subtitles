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
    assert_eq!(revision(Output), 2);
    assert_eq!(revision(TextDetect), 3);
    assert_eq!(revision(TextRead), 3);
    assert_eq!(revision(TextTrack), 2);
    assert_eq!(revision(TextTranslate), 7);
    assert_eq!(revision(TextReview), 2);
    assert_eq!(revision(TextTypeset), 2);
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

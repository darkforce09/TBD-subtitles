use job_model::job::OutputFormat;

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
    assert_eq!(revision(TextDetect), 6);
    assert_eq!(revision(TextRead), 3);
    assert_eq!(revision(TextTrack), 3);
    assert_eq!(revision(TextTranslate), 7);
    assert_eq!(revision(TextReview), 3);
    assert_eq!(revision(TextTypeset), 4);
    assert_eq!(revision(TextMask), 4);
    assert_eq!(revision(TextInpaint), 4);
    assert_eq!(revision(TextCompose), 3);
    assert_eq!(revision(TextVerify), 3);
    assert_eq!(revision(LocalizedVideo), 4);
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
                | TextVerify
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
fn a_step_reads_the_job_record_and_every_document_of_every_step_it_reads() {
    use StepName::*;
    let names = |step| -> Vec<String> {
        reads(step)
            .into_iter()
            .map(|address| {
                format!(
                    "{}/{}",
                    address.table,
                    crate::work_dir::store::kinds::shown(&address.key)
                )
            })
            .collect()
    };
    assert_eq!(
        names(Vad),
        vec!["meta/job_record", "outputs/probe_decode"],
        "the separation keeps only files"
    );
    assert_eq!(
        names(TextTranslate),
        vec![
            "meta/job_record",
            "outputs/shot_scan",
            "outputs/text_track",
            "outputs/cues",
            "outputs/cues/dropped_sounds",
            "corrections/text",
        ]
    );
    assert!(names(Review).contains(&"corrections/lines".to_string()));
    assert!(names(Output).contains(&"outputs/text_typeset/ass".to_string()));
    assert_eq!(names(ProbeDecode), vec!["meta/job_record"]);
    assert!(names(LocalizedVideo).contains(&"outputs/localized_video".to_string()));
    assert!(is_optional_read(
        Output,
        &keys::output_address(Output, None)
    ));
    assert!(!is_optional_read(Output, &keys::output_address(Cues, None)));
    for step in StepName::ALL {
        for address in reads(step) {
            assert_eq!(
                is_optional_read(step, &address),
                address.table == Table::Corrections || address == keys::output_address(step, None),
            );
            assert!(
                crate::work_dir::store::kind(address.table, &address.key).is_ok(),
                "{step} reads {address:?}, which has no kind"
            );
        }
    }
}

#[test]
fn a_steps_dependents_are_every_step_that_reads_it_through_any_path() {
    use StepName::*;
    assert_eq!(dependents(LocalizedVideo), Vec::<StepName>::new());
    assert_eq!(
        dependents(TextVerify),
        vec![Output, LocalizedVideo],
        "the typesetting reads the review, not the check"
    );
    assert_eq!(dependents(Cues), {
        vec![
            TextTranslate,
            TextReview,
            TextMask,
            TextInpaint,
            TextCompose,
            TextVerify,
            TextTypeset,
            Qc,
            Output,
            LocalizedVideo,
        ]
    });
    let all_but_the_scan: Vec<StepName> = StepName::ALL
        .into_iter()
        .filter(|step| !matches!(step, ProbeDecode | ShotScan))
        .collect();
    assert_eq!(dependents(ProbeDecode), all_but_the_scan);
    for step in StepName::ALL {
        for later in dependents(step) {
            assert!(position(later) > position(step), "{step} before {later}");
        }
    }
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

#[test]
fn the_engine_and_encoder_reach_their_steps_and_the_decoder_reaches_none() {
    use job_model::onscreen::{DetectorEngine, LocalizedEncoder};
    let base = JobSettings::with_glossary(vec![]);
    let mut decoded_on_gpu = base.clone();
    decoded_on_gpu.onscreen_text.hardware_decode = true;
    for step in StepName::ALL {
        assert_eq!(
            settings(step, &base),
            settings(step, &decoded_on_gpu),
            "{step}"
        );
    }
    let mut cuda = base.clone();
    cuda.onscreen_text.detector_engine = DetectorEngine::Cuda;
    let mut nvenc = base.clone();
    nvenc.onscreen_text.localized_encoder = LocalizedEncoder::Nvenc;
    for step in StepName::ALL {
        assert_eq!(
            settings(step, &base) != settings(step, &cuda),
            step == StepName::TextDetect,
            "{step}"
        );
        assert_eq!(
            settings(step, &base) != settings(step, &nvenc),
            step == StepName::LocalizedVideo,
            "{step}"
        );
    }
}

#[test]
fn the_output_and_the_localized_video_read_the_checked_replacements() {
    use StepName::*;
    assert_eq!(inputs(TextReview), &[ProbeDecode, ShotScan, TextTranslate]);
    assert_eq!(inputs(TextTypeset), &[TextReview]);
    assert_eq!(inputs(Output), &[Cues, TextTypeset, TextVerify]);
    assert_eq!(inputs(LocalizedVideo), &[ProbeDecode, TextVerify, Output]);
    assert_eq!(inputs(TextVerify), &[ProbeDecode, TextReview, TextCompose]);
    assert_eq!(
        keys::output_keys(TextTypeset),
        vec![
            keys::output_key(TextTypeset, None),
            keys::output_key(TextTypeset, Some(keys::TYPESET_ASS))
        ],
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
    assert_eq!(
        keys::output_keys(TextVerify),
        vec![keys::output_key(TextVerify, None)]
    );
    let mut job = JobSettings::with_glossary(Vec::new());
    let before = settings(TextVerify, &job);
    job.onscreen_text.localized_video = !job.onscreen_text.localized_video;
    assert_ne!(before, settings(TextVerify, &job));
}

#[test]
fn the_stroke_masks_own_the_frames_rows_that_composition_and_the_video_read() {
    use StepName::*;
    for step in StepName::ALL {
        let owned = writes_rows(step);
        match step {
            TextMask => assert_eq!(owned, &[Table::Frames]),
            TextVerify => assert_eq!(owned, &[Table::Readings]),
            _ => assert!(owned.is_empty(), "{step}"),
        }
        for table in reads_rows(step) {
            let owner = StepName::ALL
                .into_iter()
                .find(|s| writes_rows(*s).contains(table))
                .expect("an owner");
            assert!(dependents(owner).contains(&step), "{step} reads {table}");
        }
    }
    assert_eq!(reads_rows(TextCompose), &[Table::Frames]);
    assert_eq!(reads_rows(TextVerify), &[Table::Frames]);
    assert_eq!(reads_rows(LocalizedVideo), &[Table::Frames]);
    assert!(reads_rows(TextInpaint).is_empty());
}

#[test]
fn the_visual_lane_reads_only_the_probe_the_shot_scan_and_itself() {
    use StepName::*;
    for (index, step) in VISUAL_LANE.iter().enumerate() {
        for input in inputs(*step) {
            assert!(
                matches!(input, ProbeDecode | ShotScan) || VISUAL_LANE[..index].contains(input),
                "the lane's {step} reads {input}"
            );
        }
    }
}

#[test]
fn the_visual_lane_starts_after_the_shot_scan_and_before_its_steps() {
    let start = position(VISUAL_LANE_STARTS_AT);
    assert!(position(StepName::ShotScan) < start);
    assert!(!in_visual_lane(VISUAL_LANE_STARTS_AT));
    for step in VISUAL_LANE {
        assert!(
            start < position(step),
            "{step} comes before the lane starts"
        );
        assert!(
            !inputs(step).contains(&VISUAL_LANE_STARTS_AT),
            "{step} reads the lane's start"
        );
    }
}

#[test]
fn the_visual_lane_joins_at_the_first_step_that_reads_it() {
    let first = StepName::ALL
        .into_iter()
        .find(|step| !in_visual_lane(*step) && reads_visual_lane(*step));
    assert_eq!(first, Some(VISUAL_LANE_JOINS_AT));
    assert!(position(VISUAL_LANE_STARTS_AT) < position(VISUAL_LANE_JOINS_AT));
}

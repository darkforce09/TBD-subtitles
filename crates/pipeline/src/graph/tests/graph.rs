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
fn every_step_leaves_at_least_one_file() {
    let work = WorkDir::new("/work/job");
    for step in StepName::ALL {
        assert!(
            !outputs(step, &work, Path::new("/v/a.mp4")).is_empty(),
            "{step}"
        );
    }
    assert!(
        outputs(StepName::Output, &work, Path::new("/v/a.mp4"))
            .contains(&PathBuf::from("/v/a.srt"))
    );
}

#[test]
fn only_the_settings_a_step_reads_reach_its_fingerprint() {
    let mut a = JobSettings::with_glossary(vec![]);
    let mut b = a.clone();
    b.cut_score = 30.0;
    assert_eq!(
        settings(StepName::Alignment, &a),
        settings(StepName::Alignment, &b)
    );
    assert_ne!(settings(StepName::Cues, &a), settings(StepName::Cues, &b));
    a.glossary.push("Luffy".into());
    assert_ne!(
        settings(StepName::Adjudicate, &a),
        settings(StepName::Adjudicate, &b)
    );
}

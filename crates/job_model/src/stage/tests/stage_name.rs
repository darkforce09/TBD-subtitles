use super::*;

#[test]
fn every_stage_is_listed_once_and_parses_back_to_itself() {
    for (index, stage) in StageName::ALL.into_iter().enumerate() {
        assert_eq!(stage.as_str().parse::<StageName>(), Ok(stage));
        assert_eq!(
            StageName::ALL.iter().position(|other| *other == stage),
            Some(index),
            "{stage} is listed twice"
        );
    }
}

#[test]
fn an_unknown_name_is_refused_with_the_name() {
    let error = "subtitles".parse::<StageName>().unwrap_err();
    assert_eq!(error.to_string(), "`subtitles` is not a stage");
}

#[test]
fn json_names_match_the_command_line_names() {
    for stage in StageName::ALL {
        let json = serde_json::to_string(&stage).unwrap();
        assert_eq!(json, format!("\"{}\"", stage.as_str()));
    }
}

#[test]
fn sound_events_come_before_adjudication_which_chooses_the_cues() {
    let position = |wanted| StageName::ALL.iter().position(|s| *s == wanted).unwrap();
    assert!(position(StageName::SoundEvents) < position(StageName::Adjudication));
    assert!(position(StageName::Adjudication) < position(StageName::Alignment));
}

#[test]
fn only_model_stages_run_in_a_worker() {
    let workers: Vec<_> = StageName::ALL
        .into_iter()
        .filter(|stage| stage.runs_in_worker())
        .map(StageName::as_str)
        .collect();
    assert_eq!(
        workers,
        [
            "separation",
            "asr",
            "sound_events",
            "adjudication",
            "alignment",
            "onscreen_text"
        ]
    );
}

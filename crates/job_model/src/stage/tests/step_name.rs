use super::*;

#[test]
fn every_step_is_listed_once_and_parses_back() {
    let mut seen = std::collections::HashSet::new();
    for step in StepName::ALL {
        assert!(seen.insert(step), "{step} listed twice");
        assert_eq!(step.as_str().parse::<StepName>(), Ok(step));
    }
    assert!("asr".parse::<StepName>().is_err());
}

#[test]
fn steps_follow_the_stage_order() {
    let stages: Vec<StageName> = StepName::ALL.iter().map(|s| s.stage()).collect();
    let mut order = stages.clone();
    order.dedup();
    assert_eq!(
        order,
        StageName::ALL.to_vec(),
        "every stage once, contiguous, in order"
    );
}

#[test]
fn steps_serialise_by_name() {
    let json = serde_json::to_string(&StepName::RedecodeWhisper).expect("json");
    assert_eq!(json, "\"redecode_whisper\"");
}

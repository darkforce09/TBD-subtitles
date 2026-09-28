use super::*;

#[test]
fn every_step_belongs_to_exactly_one_stage_in_order() {
    let in_stages: Vec<StepName> = STAGES
        .iter()
        .flat_map(|stage| stage.steps.iter().copied())
        .collect();
    assert_eq!(in_stages, StepName::ALL);
    for step in StepName::ALL {
        let holding = STAGES.iter().filter(|s| s.steps.contains(&step)).count();
        assert_eq!(holding, 1, "{step}");
        assert!(stage_of(step).steps.contains(&step), "{step}");
    }
    assert_eq!(stage_of(StepName::Readjudicate).title, "Settle the words");
    assert_eq!(stage_of(StepName::Readjudicate).doing, "Settling the words");
}

#[test]
fn every_step_has_its_own_plain_title() {
    let mut titles: Vec<&str> = StepName::ALL.into_iter().map(step_title).collect();
    assert!(titles.iter().all(|t| !t.contains('_')), "{titles:?}");
    titles.sort_unstable();
    titles.dedup();
    assert_eq!(titles.len(), StepName::ALL.len());
    assert_eq!(step_title(StepName::Review), "Apply your corrections");
}

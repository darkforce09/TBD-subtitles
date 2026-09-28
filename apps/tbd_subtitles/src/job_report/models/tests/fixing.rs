use job_model::outputs::FixFamily;

use super::*;

#[test]
fn each_stage_names_its_pass_of_three() {
    assert_eq!(
        stage_words(FixStage::Reading),
        "reading the whole video (1 of 3)"
    );
    assert_eq!(
        stage_words(FixStage::Fixing(FixFamily::Timing)),
        "fixing timing and layout (2 of 3)"
    );
    assert_eq!(
        stage_words(FixStage::Checking),
        "checking each change (3 of 3)"
    );
    assert_eq!(stage_words(FixStage::Saving), "saving the changes (3 of 3)");
}

use job_model::outputs::FixFamily;

use super::*;

#[test]
fn each_stage_names_its_step_of_four() {
    assert_eq!(
        stage_words(FixStage::Reading),
        "reading the whole video (1 of 4)"
    );
    assert_eq!(
        stage_words(FixStage::Fixing(FixFamily::Timing)),
        "fixing timing and layout (2 of 4)"
    );
    assert_eq!(
        stage_words(FixStage::Checking),
        "checking each change (3 of 4)"
    );
    assert_eq!(stage_words(FixStage::Saving), "saving the changes (3 of 4)");
    assert_eq!(updating_words(), "updating the subtitles (4 of 4)");
    let steps: Vec<usize> = [
        FixStage::Reading,
        FixStage::Fixing(FixFamily::Words),
        FixStage::Checking,
        FixStage::Saving,
    ]
    .into_iter()
    .map(step_of)
    .collect();
    assert_eq!(steps, [1, 2, 3, 3]);
}

#[test]
fn a_run_and_its_correction_run_are_under_way() {
    let model = || "Claude Opus".to_string();
    let running = FixView::Running {
        model: model(),
        stage: FixStage::Reading,
        done: 0,
        total: 1,
        stopping: false,
        waiting: true,
    };
    assert!(
        running.under_way(),
        "a run waiting for a free call is under way"
    );
    assert!(FixView::Updating { model: model() }.under_way());
    assert!(!FixView::Ready { model: model() }.under_way());
    assert!(!FixView::Hidden.under_way());
}

#[test]
fn the_line_under_way_says_when_the_run_waits_for_a_free_call() {
    assert_eq!(
        running_line(3, 12, false, false),
        "3 of 12 calls done. The subtitles change only once every change is checked."
    );
    assert_eq!(
        running_line(3, 12, false, true),
        "Waiting for a free Claude call. 3 of 12 calls done."
    );
    assert_eq!(
        running_line(0, 1, true, true),
        "Stopping. Nothing is changed; Fix It again picks up where it stopped.",
        "Stop outranks the wait"
    );
}

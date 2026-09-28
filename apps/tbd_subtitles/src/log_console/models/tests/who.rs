use super::*;

#[test]
fn each_target_is_one_writer() {
    assert_eq!(Who::of("inference::llm::call_log"), Who::Ai);
    assert_eq!(Who::of("fix_it"), Who::Ai);
    assert_eq!(Who::of("pipeline::fix_it::cache"), Who::Ai);
    assert_eq!(Who::of("child_process"), Who::Program);
    assert_eq!(Who::of("job"), Who::Job);
    assert_eq!(Who::of("pipeline::runner"), Who::Job);
    assert_eq!(Who::of("stages::adjudication"), Who::Job);
    assert_eq!(Who::of("tbd_subtitles::core::portal"), Who::App);
    assert_eq!(Who::of("inference::model_store"), Who::App);
    assert_eq!(
        Who::of("jobless"),
        Who::App,
        "a name that only starts alike is not under it"
    );
}

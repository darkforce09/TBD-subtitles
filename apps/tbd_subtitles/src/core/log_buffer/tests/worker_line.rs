use super::*;

#[test]
fn a_worker_line_with_a_timestamp_gives_its_level_target_and_message() {
    let line = read(
        "tbd-subtitles[4242] 2026-09-28T10:00:00.123456Z  INFO pipeline::tasks: loading the model",
    )
    .unwrap();
    assert_eq!(line.level, Level::INFO);
    assert_eq!(line.target, "pipeline::tasks");
    assert_eq!(line.message, "loading the model");
    assert_eq!(line.call, None);
}

#[test]
fn a_call_field_is_taken_out_wherever_it_is() {
    let after = read(
        "tbd-subtitles[4242] WARN inference::llm::call_log: claude sonnet: failed call=4242-3",
    )
    .unwrap();
    assert_eq!(after.level, Level::WARN);
    assert_eq!(after.message, "claude sonnet: failed");
    assert_eq!(after.call.as_deref(), Some("4242-3"));
    let before =
        read("tbd-subtitles[1] INFO inference::llm::call_log: call=1-9 claude sonnet: done")
            .unwrap();
    assert_eq!(before.message, "claude sonnet: done");
    assert_eq!(before.call.as_deref(), Some("1-9"));
}

#[test]
fn any_other_program_line_is_left_alone() {
    assert_eq!(read("ffmpeg[7] frame=  120 fps=30"), None);
    assert_eq!(read("claude[8] Error: overloaded"), None);
    assert_eq!(read("tbd-subtitles[9] INFO no target here"), None);
    assert_eq!(read("single"), None);
}

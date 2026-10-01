use super::*;

#[test]
fn a_steps_main_document_is_named_by_the_step_and_a_further_one_by_its_part() {
    assert_eq!(
        output_key(StepName::ProbeDecode, None),
        Key::Name("probe_decode".into())
    );
    assert_eq!(
        output_key(StepName::Cues, Some(DROPPED_SOUNDS)),
        Key::Name("cues/dropped_sounds".into())
    );
    assert_eq!(
        output_keys(StepName::TextTypeset),
        vec![
            Key::Name("text_typeset".into()),
            Key::Name("text_typeset/ass".into())
        ]
    );
    assert!(output_keys(StepName::Separation).is_empty());
    assert_eq!(
        output_keys(StepName::AsrWhisper),
        vec![Key::Name("asr_whisper".into())]
    );
}

#[test]
fn every_listed_output_name_parses_back_and_nothing_else_does() {
    for step in StepName::ALL {
        for part in output_parts(step) {
            assert_eq!(
                parse_output(&output_name(step, *part)),
                Some((step, *part)),
                "{step} {part:?}"
            );
        }
    }
    assert_eq!(parse_output("separation"), None);
    assert_eq!(parse_output("cues/other"), None);
    assert_eq!(parse_output("vad/dropped_sounds"), None);
    assert_eq!(parse_output("fix"), None);
}

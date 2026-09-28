use super::*;

#[test]
fn problems_read_in_plain_words_with_their_buttons() {
    assert_eq!(
        Problem::Layout(1).title(),
        "1 subtitle breaks a layout rule"
    );
    assert_eq!(
        Problem::Layout(3).title(),
        "3 subtitles break a layout rule"
    );
    assert_eq!(
        Problem::FailedCall(1).title(),
        "1 language-model call failed"
    );
    assert_eq!(
        Problem::FailedCall(2).title(),
        "2 language-model calls failed"
    );
    assert_eq!(
        Problem::ReadingSpeed(0.912).title(),
        "Only 91.2 % of subtitles are easy to read"
    );
    assert_eq!(
        Problem::FailedCall(2).remedy().map(Remedy::label),
        Some("Try Again")
    );
    assert_eq!(
        Problem::UncoveredSpeech(84.5).remedy(),
        Some(Remedy::ShowNearbyLines(84.5)),
        "Show Nearby Lines opens at the first stretch"
    );
    assert_eq!(
        Problem::ReadingSpeed(0.9).remedy().map(Remedy::label),
        Some("Show Lines")
    );
    assert_eq!(Problem::Offset.remedy(), None);
    assert_eq!(Problem::Layout(2).remedy(), None);
}

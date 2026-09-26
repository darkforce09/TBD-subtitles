use super::*;

fn stream(position: u32, language: Option<&str>) -> AudioStream {
    AudioStream {
        index: position + 1,
        audio_position: position,
        codec: "aac".into(),
        language: language.map(String::from),
        channels: 2,
        sample_rate: 48_000,
        start_time_s: 0.0,
    }
}

#[test]
fn a_chosen_track_wins_over_the_language_tag() {
    let probe = ProbeResult {
        duration_s: 1.0,
        video: None,
        audio: vec![stream(0, Some("jpn")), stream(1, Some("eng"))],
    };
    assert_eq!(
        pick_track(&probe, None).map(|a| a.audio_position).ok(),
        Some(1)
    );
    assert_eq!(
        pick_track(&probe, Some(0)).map(|a| a.audio_position).ok(),
        Some(0)
    );
    assert!(pick_track(&probe, Some(5)).is_err());
}

#[test]
fn several_untagged_tracks_need_a_choice() {
    let probe = ProbeResult {
        duration_s: 1.0,
        video: None,
        audio: vec![stream(0, None), stream(1, None)],
    };
    assert!(matches!(
        pick_track(&probe, None),
        Err(MediaError::AmbiguousAudio(2))
    ));
}

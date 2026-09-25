use super::*;

const DRESSROSA_LIKE: &str = r#"{
  "streams": [
    {"index": 0, "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080,
     "r_frame_rate": "24/1", "start_time": "0.000000", "tags": {"language": "und"}},
    {"index": 1, "codec_type": "audio", "codec_name": "aac", "channels": 2,
     "sample_rate": "48000", "start_time": "0.000000", "tags": {"language": "und"}}
  ],
  "format": {"duration": "1853.749000"}
}"#;

#[test]
fn reads_the_video_and_audio_streams() {
    let probe = parse(DRESSROSA_LIKE).unwrap();
    assert!((probe.duration_s - 1853.749).abs() < 1e-9);
    let video = probe.video.as_ref().unwrap();
    assert_eq!((video.width, video.height), (1920, 1080));
    assert_eq!(video.fps(), Some(24.0));
    assert_eq!(probe.audio.len(), 1);
    assert_eq!(probe.audio[0].sample_rate, 48_000);
    assert_eq!(probe.audio[0].language, None, "und is no language");
}

#[test]
fn a_single_untagged_track_is_the_english_one() {
    let probe = parse(DRESSROSA_LIKE).unwrap();
    assert_eq!(english_track(&probe).unwrap().index, 1);
}

#[test]
fn the_english_tag_wins_and_positions_count_audio_only() {
    let json = r#"{"streams": [
      {"index": 0, "codec_type": "video"},
      {"index": 1, "codec_type": "audio", "tags": {"language": "jpn"}},
      {"index": 2, "codec_type": "audio", "tags": {"language": "eng"}}
    ]}"#;
    let probe = parse(json).unwrap();
    let track = english_track(&probe).unwrap();
    assert_eq!((track.index, track.audio_position), (2, 1));
}

#[test]
fn several_untagged_tracks_are_ambiguous() {
    let json = r#"{"streams": [
      {"index": 0, "codec_type": "audio"}, {"index": 1, "codec_type": "audio"}
    ]}"#;
    let probe = parse(json).unwrap();
    assert!(matches!(
        english_track(&probe),
        Err(MediaError::AmbiguousAudio(2))
    ));
}

#[test]
fn no_audio_is_reported() {
    let probe = parse(r#"{"streams": []}"#).unwrap();
    assert!(matches!(english_track(&probe), Err(MediaError::NoAudio)));
}

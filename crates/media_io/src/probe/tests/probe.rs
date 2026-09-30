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

#[test]
fn the_pixel_format_and_colour_tags_are_read() {
    let json = r#"{"streams": [
      {"index": 0, "codec_type": "video", "codec_name": "hevc", "width": 1920, "height": 1080,
       "r_frame_rate": "24000/1001", "pix_fmt": "yuv420p10le", "color_range": "tv",
       "color_space": "bt709", "color_transfer": "bt709", "color_primaries": "bt709"}
    ]}"#;
    let video = parse(json).unwrap().video.unwrap();
    assert_eq!(video.pix_fmt.as_deref(), Some("yuv420p10le"));
    assert_eq!(video.color_primaries.as_deref(), Some("bt709"));
    assert_eq!(video.color_transfer.as_deref(), Some("bt709"));
    assert_eq!(video.color_space.as_deref(), Some("bt709"));
    assert_eq!(video.color_range.as_deref(), Some("tv"));
    assert_eq!((video.frame_rate_num, video.frame_rate_den), (24000, 1001));
}

#[test]
fn unknown_or_missing_colour_tags_are_no_tags() {
    let json = r#"{"streams": [
      {"index": 0, "codec_type": "video", "pix_fmt": "yuv420p", "color_range": "unknown",
       "color_primaries": "unspecified", "color_transfer": "reserved", "color_space": ""}
    ]}"#;
    let video = parse(json).unwrap().video.unwrap();
    assert_eq!(video.pix_fmt.as_deref(), Some("yuv420p"));
    assert_eq!(
        (
            video.color_primaries,
            video.color_transfer,
            video.color_space,
            video.color_range
        ),
        (None, None, None, None)
    );
    let bare = parse(DRESSROSA_LIKE).unwrap().video.unwrap();
    assert_eq!(bare.pix_fmt, None);
}

#[test]
fn a_probe_saved_before_the_colour_fields_still_reads() {
    let saved = r#"{"index": 0, "codec": "h264", "width": 1920, "height": 1080,
      "frame_rate_num": 24, "frame_rate_den": 1, "start_time_s": 0.0}"#;
    let video: VideoStream = serde_json::from_str(saved).unwrap();
    assert_eq!(
        (
            video.width,
            video.pix_fmt.clone(),
            video.color_range.clone()
        ),
        (1920, None, None)
    );
    let written = serde_json::to_string(&video).unwrap();
    assert_eq!(
        serde_json::from_str::<VideoStream>(&written).unwrap(),
        video
    );
}

#[test]
fn the_stream_bit_rate_is_read_else_the_files() {
    let own = r#"{"streams": [{"index": 0, "codec_type": "video", "bit_rate": "2623912"}],
      "format": {"bit_rate": "2791817"}}"#;
    assert_eq!(parse(own).unwrap().video.unwrap().bit_rate, Some(2_623_912));
    let file = r#"{"streams": [{"index": 0, "codec_type": "video"}],
      "format": {"bit_rate": "2791817"}}"#;
    assert_eq!(
        parse(file).unwrap().video.unwrap().bit_rate,
        Some(2_791_817)
    );
    let none = r#"{"streams": [{"index": 0, "codec_type": "video", "bit_rate": "N/A"}]}"#;
    assert_eq!(parse(none).unwrap().video.unwrap().bit_rate, None);
}

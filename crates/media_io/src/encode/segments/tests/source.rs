use super::*;

/// ffprobe's JSON for a Matroska H.264 Main 4.0 stream with B-frames, as mkvmerge tags it.
const MATROSKA_MAIN: &str = r#"{
    "streams": [{
        "index": 0, "codec_name": "h264", "profile": "Main", "codec_type": "video",
        "width": 1920, "height": 1080, "has_b_frames": 2, "sample_aspect_ratio": "1:1",
        "pix_fmt": "yuv420p", "level": 40, "color_range": "tv", "color_space": "bt709",
        "color_transfer": "bt709", "color_primaries": "unknown", "field_order": "progressive",
        "refs": 4, "is_avc": "true", "nal_length_size": "4",
        "r_frame_rate": "24000/1001", "avg_frame_rate": "24000/1001",
        "tags": {"BPS": "3612345", "DURATION": "00:23:40.000000000"}
    }],
    "format": {"bit_rate": "4012345"}
}"#;

fn eligible_source() -> H264Source {
    parse_source(MATROSKA_MAIN).unwrap()
}

/// A constant 24000/1001 timeline of `frames` frames.
fn timeline(frames: usize) -> Vec<(f64, f64)> {
    let frame = 1001.0 / 24000.0;
    (0..frames)
        .map(|i| (i as f64 * frame, (i + 1) as f64 * frame))
        .collect()
}

#[test]
fn a_matroska_stream_reads_into_what_a_segment_must_match() {
    let source = eligible_source();
    assert_eq!(source.codec, "h264");
    assert_eq!(source.profile, Some(H264Profile::Main));
    assert_eq!(source.level, H264Level(40));
    assert_eq!((source.refs, source.reorder_depth), (4, 2));
    assert_eq!((source.width, source.height), (1920, 1080));
    assert_eq!(source.sample_aspect_ratio, Some((1, 1)));
    assert_eq!(source.pix_fmt.as_deref(), Some("yuv420p"));
    assert_eq!(source.colour.primaries, None);
    assert_eq!(source.colour.matrix.as_deref(), Some("bt709"));
    assert_eq!(source.colour.range.as_deref(), Some("tv"));
    assert_eq!(source.frame_rate, (24000, 1001));
    assert_eq!(source.field_order.as_deref(), Some("progressive"));
    assert_eq!(source.packet_format, Some(PacketFormat::LengthPrefixed(4)));
    // The stream has no rate of its own; mkvmerge's statistic is the video's, not the file's.
    assert_eq!(source.bit_rate, Some(3_612_345));
    assert!((source.frame_s() - 1001.0 / 24000.0).abs() < 1e-12);
}

#[test]
fn transport_streams_are_annex_b_and_unknown_values_stay_unknown() {
    let json = r#"{"streams": [{"codec_name": "h264", "profile": "High", "level": -99,
        "is_avc": "false", "nal_length_size": "0", "sample_aspect_ratio": "0:1",
        "r_frame_rate": "25/1"}], "format": {"bit_rate": "5000000"}}"#;
    let source = parse_source(json).unwrap();
    assert_eq!(source.packet_format, Some(PacketFormat::AnnexB));
    assert_eq!(source.level, H264Level(0));
    assert_eq!(source.sample_aspect_ratio, None);
    assert_eq!(source.bit_rate, Some(5_000_000));
    assert_eq!(source.pix_fmt, None);
    let bare = parse_source(r#"{"streams": [{"codec_name": "h264"}]}"#).unwrap();
    assert_eq!(bare.packet_format, None);
    assert_eq!(bare.fps(), 0.0);
    assert_eq!(bare.frame_s(), 0.0);
    assert!(parse_source(r#"{"streams": []}"#).is_err());
    assert!(parse_source("not json").is_err());
}

#[test]
fn the_first_frame_and_the_first_audio_packet_come_from_the_leading_packets() {
    let video = concat!(
        "pts_time=-0.083333|flags=KD_\n",
        "pts_time=0.125000|flags=___\n",
        "pts_time=0.041667|flags=___\n",
        "pts_time=0.083333|flags=___\n",
        "pts_time=N/A|flags=___\n",
    );
    assert_eq!(first_presentation(video, true), Some(0.041667));
    assert_eq!(first_presentation(video, false), Some(-0.083333));
    let audio = "pts_time=-0.023220|flags=KD_\n";
    assert_eq!(first_presentation(audio, false), Some(-0.02322));
    assert_eq!(first_presentation("", false), None);
    let starts = StreamStarts {
        video_s: Some(0.5),
        audio_s: Some(-0.023),
    };
    assert!((starts.video_after_audio_s().unwrap() - 0.523).abs() < 1e-9);
    let silent = StreamStarts {
        video_s: Some(0.5),
        audio_s: None,
    };
    assert_eq!(silent.video_after_audio_s(), None);
}

#[test]
fn a_progressive_constant_rate_h264_source_is_eligible() {
    let source = eligible_source();
    assert_eq!(
        segment_eligibility(&source, &timeline(48), &[0, 24]),
        Ok(())
    );
    let mut ten_bit = source.clone();
    ten_bit.profile = Some(H264Profile::High10);
    ten_bit.pix_fmt = Some("yuv420p10le".into());
    assert_eq!(segment_eligibility(&ten_bit, &timeline(48), &[0]), Ok(()));
}

/// A change made to an eligible source.
type Change<'a> = &'a dyn Fn(&mut H264Source);

#[test]
fn every_other_source_says_why_it_is_encoded_whole() {
    let source = eligible_source();
    let frames = timeline(48);
    let refused = |change: Change, keyframes: &[u64]| {
        let mut changed = source.clone();
        change(&mut changed);
        segment_eligibility(&changed, &frames, keyframes)
            .unwrap_err()
            .0
    };
    let cases: [(Change, &str); 9] = [
        (&|s| s.codec = "hevc".into(), "the video is hevc, not H.264"),
        (
            &|s| {
                s.profile = None;
                s.profile_name = "High 4:2:2".into()
            },
            "the H.264 profile High 4:2:2 has no segment encoder",
        ),
        (
            &|s| s.pix_fmt = Some("yuv420p10le".into()),
            "the pixel format yuv420p10le is not yuv420p",
        ),
        (
            &|s| s.field_order = Some("tt".into()),
            "the video is interlaced",
        ),
        (&|s| s.level = H264Level(0), "the H.264 level 0 is unknown"),
        (
            &|s| s.packet_format = None,
            "the H.264 packet format is unknown",
        ),
        (&|s| s.width = 1919, "the frame size 1919x1080 is not even"),
        (
            &|s| s.average_frame_rate = (30000, 1001),
            "the video has a variable frame rate",
        ),
        (
            &|s| s.frame_rate = (0, 0),
            "the video has a variable frame rate",
        ),
    ];
    for (change, reason) in cases {
        assert_eq!(refused(change, &[0]), reason);
    }
    assert_eq!(
        refused(&|_| {}, &[12]),
        "the video does not start on a keyframe"
    );
    let mut drifting = frames.clone();
    drifting[30].0 += 0.02;
    assert!(segment_eligibility(&source, &drifting, &[0]).is_err());
}

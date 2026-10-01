use super::*;
use crate::encode::segments::StreamStarts;
use crate::encode::segments::nal::PacketFormat;

/// A Main 4.0 1080p source at 3.6 Mb/s with B-frames and four reference frames.
fn source() -> H264Source {
    H264Source {
        codec: "h264".into(),
        profile_name: "Main".into(),
        profile: Some(H264Profile::Main),
        level: H264Level(40),
        refs: 4,
        reorder_depth: 2,
        pix_fmt: Some("yuv420p".into()),
        width: 1920,
        height: 1080,
        sample_aspect_ratio: Some((1, 1)),
        colour: VideoColour {
            primaries: Some("bt709".into()),
            transfer: None,
            matrix: Some("bt709".into()),
            range: Some("tv".into()),
        },
        frame_rate: (24000, 1001),
        average_frame_rate: (24000, 1001),
        field_order: Some("progressive".into()),
        packet_format: Some(PacketFormat::LengthPrefixed(4)),
        bit_rate: Some(3_600_000),
        starts: StreamStarts::default(),
    }
}

fn spec(encoder: LocalizedEncoder) -> SegmentSpec {
    SegmentSpec::for_source(
        &source(),
        PixelFormat::Yuv420p,
        encoder,
        PathBuf::from("/job/encode00001.mkv"),
    )
    .unwrap()
}

fn joined(args: &[String]) -> String {
    args.join(" ")
}

#[test]
fn copied_pieces_are_cut_at_their_first_frames_with_headers_in_band() {
    let args = copy_args(
        Path::new("/videos/ep 11.mkv"),
        &[24, 72],
        240,
        Path::new("/job/pieces"),
    );
    assert_eq!(
        joined(&args),
        "-nostdin -hide_banner -v error -y -i /videos/ep 11.mkv -map 0:v:0 -an -sn -dn -c copy \
         -bsf:v h264_mp4toannexb,dump_extra=freq=keyframe -f segment -segment_format matroska \
         -segment_frames 24,72 /job/pieces/copy%05d.mkv"
    );
    let whole = copy_args(Path::new("/v.mkv"), &[], 240, Path::new("/job"));
    assert!(joined(&whole).contains("-segment_frames 240 "), "{whole:?}");
    assert_eq!(copied_piece_name(3), "copy00003.mkv");
}

#[test]
fn an_x264_segment_matches_the_source_and_repeats_its_headers() {
    let spec = spec(LocalizedEncoder::X264);
    assert_eq!(spec.preset, X264_SEGMENT_PRESET);
    assert_eq!(
        joined(&segment_args(&spec)),
        "-hide_banner -v error -y -f rawvideo -pix_fmt yuv420p -s 1920x1080 \
         -framerate 24000/1001 -i pipe:0 -map 0:v:0 -an -sn -dn -vf setsar=1/1 \
         -c:v libx264 -preset slow -crf 16 -profile:v main -level 4.0 \
         -x264-params stitchable=1:repeat-headers=1:open-gop=0:ref=4:bframes=3:b-pyramid=none \
         -forced-idr 1 -maxrate 5400000 -bufsize 10800000 -pix_fmt yuv420p \
         -color_primaries bt709 -colorspace bt709 -color_range tv -f matroska \
         /job/encode00001.mkv"
    );
}

#[test]
fn an_nvenc_segment_puts_its_headers_in_band_with_dump_extra() {
    let spec = spec(LocalizedEncoder::Nvenc);
    assert_eq!(spec.preset, NVENC_SEGMENT_PRESET);
    let args = joined(&segment_args(&spec));
    assert!(
        args.contains(
            "-c:v h264_nvenc -preset p7 -tune hq -rc vbr -cq 19 -b:v 0 -profile:v main \
             -level 4.0 -bf 3 -b_ref_mode disabled -refs 4 -forced-idr 1 \
             -bsf:v dump_extra=freq=keyframe -maxrate 5400000 -bufsize 10800000"
        ),
        "{args}"
    );
    assert!(!args.contains("repeat_headers"), "{args}");
}

#[test]
fn a_ten_bit_source_is_always_encoded_by_x264_in_high_10() {
    let mut ten_bit = source();
    ten_bit.profile = Some(H264Profile::High10);
    ten_bit.profile_name = "High 10".into();
    ten_bit.pix_fmt = Some("yuv420p10le".into());
    let spec = SegmentSpec::for_source(
        &ten_bit,
        PixelFormat::Yuv420p10le,
        LocalizedEncoder::Nvenc,
        PathBuf::from("/job/encode00000.mkv"),
    )
    .unwrap();
    assert_eq!(spec.encoder, LocalizedEncoder::X264);
    let args = joined(&segment_args(&spec));
    assert!(args.contains("-pix_fmt yuv420p10le -s 1920x1080"), "{args}");
    assert!(args.contains("-profile:v high10"), "{args}");
    assert!(args.ends_with("-pix_fmt yuv420p10le -color_primaries bt709 -colorspace bt709 -color_range tv -f matroska /job/encode00000.mkv"), "{args}");
    // NVENC is never even tried for it.
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    assert_eq!(
        segment_encoder(&programs, LocalizedEncoder::Nvenc, &ten_bit),
        LocalizedEncoder::X264
    );
}

#[test]
fn a_baseline_source_without_b_frames_gets_none_and_no_aspect_filter_when_unstated() {
    let mut baseline = source();
    baseline.profile = Some(H264Profile::Baseline);
    baseline.reorder_depth = 0;
    baseline.refs = 0;
    baseline.sample_aspect_ratio = None;
    baseline.bit_rate = None;
    let spec = SegmentSpec::for_source(
        &baseline,
        PixelFormat::Yuv420p,
        LocalizedEncoder::X264,
        PathBuf::from("/job/encode00000.mkv"),
    )
    .unwrap();
    assert_eq!((spec.b_frames, spec.refs), (0, 1));
    let args = joined(&segment_args(&spec));
    assert!(args.contains("ref=1:bframes=0:"), "{args}");
    assert!(!args.contains("setsar"), "{args}");
    // Without a source rate the level's own limits cap it.
    assert!(
        args.contains("-maxrate 20000000 -bufsize 25000000"),
        "{args}"
    );
}

#[test]
fn a_segment_spec_refuses_frames_or_streams_it_cannot_match() {
    let wrong_depth = SegmentSpec::for_source(
        &source(),
        PixelFormat::Yuv420p10le,
        LocalizedEncoder::X264,
        PathBuf::from("/job/e.mkv"),
    );
    assert_eq!(
        wrong_depth.unwrap_err().0,
        "Main segments take yuv420p frames, not yuv420p10le"
    );
    let mut unmatched = source();
    unmatched.profile = None;
    unmatched.profile_name = "High 4:4:4 Predictive".into();
    let refused = SegmentSpec::for_source(
        &unmatched,
        PixelFormat::Yuv420p,
        LocalizedEncoder::X264,
        "/e".into(),
    );
    assert!(refused.is_err());
    let mut unknown_level = source();
    unknown_level.level = H264Level(0);
    let refused = SegmentSpec::for_source(
        &unknown_level,
        PixelFormat::Yuv420p,
        LocalizedEncoder::X264,
        "/e".into(),
    );
    assert_eq!(refused.unwrap_err().0, "the H.264 level 0 is unknown");
}

#[test]
fn nvenc_is_used_only_when_asked_and_it_runs() {
    let absent = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    assert_eq!(
        segment_encoder(&absent, LocalizedEncoder::X264, &source()),
        LocalizedEncoder::X264
    );
    assert_eq!(
        segment_encoder(&absent, LocalizedEncoder::Nvenc, &source()),
        LocalizedEncoder::X264
    );
    // `true` stands in for an FFmpeg whose one-frame NVENC encode succeeds.
    let runs = Programs {
        ffmpeg: "true".into(),
        ..Programs::default()
    };
    assert_eq!(
        segment_encoder(&runs, LocalizedEncoder::Nvenc, &source()),
        LocalizedEncoder::Nvenc
    );
}

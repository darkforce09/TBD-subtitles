use super::*;

fn spec(encoder: Encoder, pixel_format: PixelFormat, colour: VideoColour) -> EncodeSpec {
    EncodeSpec {
        width: 1920,
        height: 1080,
        frame_rate: (24000, 1001),
        pixel_format,
        video_offset_s: 0.042,
        colour,
        source_bit_rate: None,
        source: PathBuf::from("/videos/Dressrosa 11.mkv"),
        output: PathBuf::from("/videos/Dressrosa 11.localized.mkv.part"),
        encoder,
    }
}

fn bt709() -> VideoColour {
    VideoColour {
        primaries: Some("bt709".into()),
        transfer: Some("bt709".into()),
        matrix: Some("bt709".into()),
        range: Some("tv".into()),
    }
}

/// The arguments every encode shares, around the codec and colour arguments.
fn expected(format: &str, codec: &[&str], colour: &[&str]) -> Vec<String> {
    let mut args = vec![
        "-hide_banner",
        "-v",
        "error",
        "-y",
        "-f",
        "rawvideo",
        "-pix_fmt",
        format,
        "-s",
        "1920x1080",
        "-framerate",
        "24000/1001",
        "-itsoffset",
        "0.042000",
        "-i",
        "pipe:0",
        "-i",
        "/videos/Dressrosa 11.mkv",
        "-map",
        "0:v:0",
        "-map",
        "1:a?",
        "-map_chapters",
        "1",
        "-map_metadata",
        "1",
        "-sn",
        "-dn",
        "-c:a",
        "copy",
    ];
    args.extend(codec);
    args.extend(colour);
    args.extend([
        "-max_muxing_queue_size",
        "4096",
        "-f",
        "matroska",
        "/videos/Dressrosa 11.localized.mkv.part",
    ]);
    args.into_iter().map(String::from).collect()
}

const NVENC: [&str; 12] = [
    "-c:v",
    "hevc_nvenc",
    "-preset",
    "p6",
    "-tune",
    "hq",
    "-rc",
    "vbr",
    "-cq",
    "19",
    "-b:v",
    "0",
];

const BT709: [&str; 8] = [
    "-color_primaries",
    "bt709",
    "-color_trc",
    "bt709",
    "-colorspace",
    "bt709",
    "-color_range",
    "tv",
];

#[test]
fn nvenc_writes_main_for_8_bit_and_main10_for_10_bit() {
    let mut main: Vec<&str> = NVENC.to_vec();
    main.extend(["-profile:v", "main"]);
    assert_eq!(
        encode_args(&spec(Encoder::HevcNvenc, PixelFormat::Yuv420p, bt709())),
        expected("yuv420p", &main, &BT709)
    );
    let mut main10: Vec<&str> = NVENC.to_vec();
    main10.extend(["-profile:v", "main10"]);
    assert_eq!(
        encode_args(&spec(Encoder::HevcNvenc, PixelFormat::Yuv420p10le, bt709())),
        expected("yuv420p10le", &main10, &BT709)
    );
}

#[test]
fn libx264_writes_constant_quality_and_high10_for_10_bit() {
    let x264 = ["-c:v", "libx264", "-preset", "slow", "-crf", "16"];
    assert_eq!(
        encode_args(&spec(
            Encoder::Libx264,
            PixelFormat::Yuv420p,
            VideoColour::default()
        )),
        expected("yuv420p", &x264, &[])
    );
    let mut high10 = x264.to_vec();
    high10.extend(["-profile:v", "high10"]);
    assert_eq!(
        encode_args(&spec(
            Encoder::Libx264,
            PixelFormat::Yuv420p10le,
            VideoColour::default()
        )),
        expected("yuv420p10le", &high10, &[])
    );
}

#[test]
fn rgb_frames_are_encoded_as_4_2_0() {
    let mut x264 = vec!["-c:v", "libx264", "-preset", "slow", "-crf", "16"];
    x264.extend(["-pix_fmt", "yuv420p"]);
    assert_eq!(
        encode_args(&spec(
            Encoder::Libx264,
            PixelFormat::Rgb24,
            VideoColour::default()
        )),
        expected("rgb24", &x264, &[])
    );
}

#[test]
fn only_known_colour_tags_are_written() {
    let partial = VideoColour {
        primaries: None,
        transfer: Some("smpte2084".into()),
        matrix: None,
        range: Some("pc".into()),
    };
    assert_eq!(
        colour_args(&partial),
        ["-color_trc", "smpte2084", "-color_range", "pc"].map(String::from)
    );
    assert!(colour_args(&VideoColour::default()).is_empty());
}

#[test]
fn the_colour_comes_from_the_probed_stream() {
    let stream = VideoStream {
        index: 0,
        codec: "h264".into(),
        width: 1920,
        height: 1080,
        frame_rate_num: 24,
        frame_rate_den: 1,
        start_time_s: 0.0,
        pix_fmt: Some("yuv420p".into()),
        color_primaries: Some("bt709".into()),
        color_transfer: None,
        color_space: Some("bt709".into()),
        color_range: Some("tv".into()),
        bit_rate: None,
    };
    assert_eq!(
        VideoColour::of(&stream),
        VideoColour {
            primaries: Some("bt709".into()),
            transfer: None,
            matrix: Some("bt709".into()),
            range: Some("tv".into()),
        }
    );
}

#[test]
fn encoder_names_and_the_nvenc_test_are_exact() {
    assert_eq!(Encoder::HevcNvenc.name(), "hevc_nvenc");
    assert_eq!(Encoder::Libx264.name(), "libx264");
    let test = nvenc_test_args();
    assert!(
        test.windows(2)
            .any(|pair| pair == ["-i", "color=size=256x256:rate=24"])
    );
    assert!(test.windows(2).any(|pair| pair == ["-c:v", "hevc_nvenc"]));
    assert!(test.windows(2).any(|pair| pair == ["-frames:v", "1"]));
    assert_eq!(&test[test.len() - 3..], ["-f", "null", "-"]);
}

#[test]
fn the_encoder_list_is_matched_by_exact_name() {
    let listing = "Encoders:\n V..... = Video\n ------\n \
                   V....D libx264rgb           libx264 H.264 RGB (codec h264)\n \
                   V....D hevc_nvenc           NVIDIA NVENC hevc encoder (codec hevc)\n";
    assert!(lists_encoder(listing, "hevc_nvenc"));
    assert!(!lists_encoder(listing, "libx264"));
    assert!(!lists_encoder("", "libx264"));
}

/// `count` frames at `fps` from `offset`, each ending where the next begins.
fn constant(count: usize, fps: f64, offset: f64) -> Vec<(f64, f64)> {
    (0..count)
        .map(|i| (offset + i as f64 / fps, offset + (i + 1) as f64 / fps))
        .collect()
}

#[test]
fn a_constant_rate_allows_millisecond_rounding_and_a_short_last_frame() {
    let fps = 24000.0 / 1001.0;
    let mut timeline = constant(2000, fps, 0.042);
    assert!(is_constant_frame_rate(&timeline, fps));
    // Matroska stores milliseconds: 41 and 42 ms frames, 1.7 % off one by one, still constant.
    let rounded: Vec<(f64, f64)> = timeline
        .iter()
        .map(|&(a, b)| ((a * 1000.0).round() / 1000.0, (b * 1000.0).round() / 1000.0))
        .collect();
    assert!(
        rounded
            .windows(2)
            .any(|w| (w[0].1 - w[0].0 - 0.041).abs() < 1e-9)
    );
    assert!(is_constant_frame_rate(&rounded, fps));
    let last = timeline.len() - 1;
    timeline[last].1 = timeline[last].0 + 0.3 / fps;
    assert!(is_constant_frame_rate(&timeline, fps));
    assert!(is_constant_frame_rate(&[], fps));
    assert!(is_constant_frame_rate(&[(0.0, 5.0)], fps));
    assert!(!is_constant_frame_rate(&[(0.0, 0.1)], 0.0));
    assert!(!is_constant_frame_rate(&[(0.0, 0.1)], f64::NAN));
}

#[test]
fn a_displaced_frame_or_a_drift_is_not_constant() {
    let fps = 24.0;
    let mut displaced = constant(100, fps, 0.0);
    displaced[50].0 += 0.2 / fps;
    assert!(!is_constant_frame_rate(&displaced, fps));
    // 24000/1001 frames read as 24 fps drift a frame every 1001 frames.
    let drifting = constant(2000, 24000.0 / 1001.0, 0.0);
    assert!(!is_constant_frame_rate(&drifting, fps));
}

#[test]
fn a_variable_rate_timeline_is_not_constant() {
    let timeline = [
        (0.0, 1.0 / 24.0),
        (1.0 / 24.0, 3.0 / 24.0),
        (3.0 / 24.0, 0.2),
    ];
    assert!(!is_constant_frame_rate(&timeline, 24.0));
}

#[test]
fn a_known_source_rate_caps_the_peak_rate_per_encoder() {
    let mut hevc = spec(
        Encoder::HevcNvenc,
        PixelFormat::Yuv420p,
        VideoColour::default(),
    );
    hevc.source_bit_rate = Some(2_624_000);
    let args = encode_args(&hevc);
    let after = |flag: &str| {
        let at = args.iter().position(|a| a == flag).expect(flag);
        args[at + 1].clone()
    };
    assert_eq!(after("-maxrate"), "3280000");
    assert_eq!(after("-bufsize"), "6560000");
    let mut h264 = spec(
        Encoder::Libx264,
        PixelFormat::Yuv420p,
        VideoColour::default(),
    );
    h264.source_bit_rate = Some(2_000_000);
    let args = encode_args(&h264);
    let at = args.iter().position(|a| a == "-maxrate").expect("-maxrate");
    assert_eq!(args[at + 1], "3000000");
    let unknown = spec(
        Encoder::HevcNvenc,
        PixelFormat::Yuv420p,
        VideoColour::default(),
    );
    assert!(!encode_args(&unknown).iter().any(|a| a == "-maxrate"));
}

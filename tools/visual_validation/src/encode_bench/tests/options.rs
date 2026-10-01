use super::*;
use clap::Parser;
use media_io::encode::VideoColour;
use media_io::encode::segments::{H264Level, H264Profile, PacketFormat, StreamStarts};

/// A High 4.1 1080p source with B-frames and four reference frames, in `profile`.
fn source(profile: H264Profile, pix_fmt: &str) -> H264Source {
    H264Source {
        codec: "h264".into(),
        profile_name: "High".into(),
        profile: Some(profile),
        level: H264Level(41),
        refs: 4,
        reorder_depth: 2,
        pix_fmt: Some(pix_fmt.into()),
        width: 1920,
        height: 1080,
        sample_aspect_ratio: Some((1, 1)),
        colour: VideoColour::default(),
        frame_rate: (24000, 1001),
        average_frame_rate: (24000, 1001),
        field_order: Some("progressive".into()),
        packet_format: Some(PacketFormat::LengthPrefixed(4)),
        bit_rate: Some(4_000_000),
        starts: StreamStarts::default(),
    }
}

#[test]
fn a_segment_row_runs_the_production_command_with_only_its_preset_changed() {
    let output = Path::new("/bench/libx264-veryfast.mkv");
    let high = source(H264Profile::High, "yuv420p");
    let spec = segment_spec(
        &high,
        PixelFormat::Yuv420p,
        LocalizedEncoder::X264,
        "veryfast",
        output,
    )
    .unwrap();
    let mut production = SegmentSpec::for_source(
        &high,
        PixelFormat::Yuv420p,
        LocalizedEncoder::X264,
        output.to_path_buf(),
    )
    .unwrap();
    assert_eq!(production.preset, X264_SEGMENT_PRESET);
    production.preset = "veryfast".into();
    assert_eq!(spec, production);
    let args = segment_args(&spec).join(" ");
    assert!(args.contains("-c:v libx264 -preset veryfast"), "{args}");
    assert!(args.contains("stitchable=1:repeat-headers=1"), "{args}");

    let nvenc = segment_spec(
        &high,
        PixelFormat::Yuv420p,
        LocalizedEncoder::Nvenc,
        "p3",
        output,
    )
    .unwrap();
    assert!(
        segment_args(&nvenc)
            .join(" ")
            .contains("-c:v h264_nvenc -preset p3 -tune hq")
    );
}

#[test]
fn a_ten_bit_source_has_no_nvenc_segment_row() {
    let high10 = source(H264Profile::High10, "yuv420p10le");
    let output = Path::new("/bench/h264_nvenc-p1.mkv");
    let refused = segment_spec(
        &high10,
        PixelFormat::Yuv420p10le,
        LocalizedEncoder::Nvenc,
        "p1",
        output,
    );
    assert!(refused.unwrap_err().starts_with("n/a:"));
    let refused = segment_spec(
        &high10,
        PixelFormat::Yuv420p,
        LocalizedEncoder::X264,
        "slow",
        output,
    );
    assert!(
        refused.is_err(),
        "8-bit frames do not fit a High 10 segment"
    );
}

#[derive(Parser)]
struct Command {
    #[command(flatten)]
    options: Options,
}

fn parse(args: &[&str]) -> Result<Options, clap::Error> {
    Command::try_parse_from(std::iter::once("encode-bench").chain(args.iter().copied()))
        .map(|command| command.options)
}

#[test]
fn defaults_measure_a_minute_from_ten_minutes() {
    let options = parse(&["/media/ep.mkv"]).expect("a video alone parses");
    assert_eq!(options.video, PathBuf::from("/media/ep.mkv"));
    assert_eq!(options.start, 600.0);
    assert_eq!(options.duration, 60.0);
    assert_eq!(options.ffmpeg_dir, None);
    assert_eq!(options.out_dir, None);
    let clip = options.clip().expect("the default clip is usable");
    assert_eq!(clip.start_s, 600.0);
    assert_eq!(clip.duration_s, 60.0);
}

#[test]
fn every_option_is_read() {
    let options = parse(&[
        "ep.mkv",
        "--start",
        "30",
        "--duration",
        "12.5",
        "--ffmpeg-dir",
        "/opt/ffmpeg",
        "--out-dir",
        "/tmp/bench",
    ])
    .expect("every option parses");
    assert_eq!(options.start, 30.0);
    assert_eq!(options.duration, 12.5);
    assert_eq!(options.ffmpeg_dir, Some(PathBuf::from("/opt/ffmpeg")));
    assert_eq!(options.out_dir, Some(PathBuf::from("/tmp/bench")));
}

#[test]
fn a_missing_video_or_an_unusable_clip_is_refused() {
    assert!(parse(&[]).is_err());
    assert!(parse(&["ep.mkv", "--duration", "long"]).is_err());
    let negative = parse(&["ep.mkv", "--start=-1"]).expect("a negative start parses");
    assert!(negative.clip().is_err());
    let empty = parse(&["ep.mkv", "--duration", "0"]).expect("a zero duration parses");
    assert!(empty.clip().is_err());
}

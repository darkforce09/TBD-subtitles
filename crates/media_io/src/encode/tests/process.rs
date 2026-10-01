use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use child_process::RunError;
use job_model::onscreen::LocalizedEncoder;

use super::*;
use crate::encode::segments::{H264Level, H264Profile, PeakRate};
use crate::encode::{Encoder, VideoColour, available_encoder};
use crate::video_frames::{Decode, FrameStream, PixelFormat, timeline};

fn spec(dir: &Path, encoder: Encoder) -> EncodeSpec {
    EncodeSpec {
        width: 256,
        height: 144,
        frame_rate: (24, 1),
        pixel_format: PixelFormat::Yuv420p,
        video_offset_s: 0.0,
        colour: VideoColour::default(),
        source_bit_rate: None,
        source: dir.join("source.mkv"),
        output: dir.join("localized.mkv"),
        encoder,
    }
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("encode-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_tail_keeps_the_end_at_a_character_boundary() {
    assert_eq!(tail("short", 10), "short");
    assert_eq!(tail("abcdef", 3), "def");
    assert_eq!(tail("aé", 1), "");
    assert_eq!(tail("aéb", 2), "b");
}

#[test]
fn an_invalid_spec_is_refused_before_anything_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let dir = Path::new("/videos");
    let timeout = Duration::from_secs(10);
    let mut odd = spec(dir, Encoder::Libx264);
    odd.width = 255;
    let mut no_rate = spec(dir, Encoder::Libx264);
    no_rate.frame_rate = (24, 0);
    let mut offset = spec(dir, Encoder::Libx264);
    offset.video_offset_s = f64::INFINITY;
    let mut in_place = spec(dir, Encoder::Libx264);
    in_place.output = in_place.source.clone();
    for bad in [odd, no_rate, offset, in_place] {
        assert!(matches!(
            EncoderProcess::start(&programs, &bad, timeout, None),
            Err(MediaError::Parse(_))
        ));
    }
}

/// A 256x144 Main 3.0 segment spec with no source behind it.
fn segment(dir: &Path) -> SegmentSpec {
    SegmentSpec {
        width: 256,
        height: 144,
        frame_rate: (24, 1),
        pixel_format: PixelFormat::Yuv420p,
        encoder: LocalizedEncoder::X264,
        preset: "slow".into(),
        profile: H264Profile::Main,
        level: H264Level(30),
        refs: 1,
        b_frames: 3,
        sample_aspect_ratio: None,
        colour: VideoColour::default(),
        peak: PeakRate {
            max_bits_per_s: 1_000_000,
            buffer_bits: 2_000_000,
        },
        output: dir.join("encode00001.mkv"),
    }
}

#[test]
fn an_invalid_segment_is_refused_before_anything_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let dir = Path::new("/job");
    let mut odd = segment(dir);
    odd.height = 143;
    let mut no_rate = segment(dir);
    no_rate.frame_rate = (0, 1);
    for bad in [odd, no_rate] {
        assert!(matches!(
            EncoderProcess::start_segment(&programs, &bad, Duration::from_secs(10), None),
            Err(MediaError::Parse(_))
        ));
    }
    // `true` stands in for FFmpeg: a valid segment starts and takes whole frames.
    let stand_in = Programs {
        ffmpeg: "true".into(),
        ..Programs::default()
    };
    let process =
        EncoderProcess::start_segment(&stand_in, &segment(dir), Duration::from_secs(10), None)
            .unwrap();
    assert_eq!(process.frame_bytes(), 256 * 144 * 3 / 2);
    assert_eq!(process.finish().unwrap(), 0);
}

#[test]
fn a_frame_of_the_wrong_size_is_refused() {
    // `true` stands in for FFmpeg: it ignores the arguments and exits cleanly unread.
    let programs = Programs {
        ffmpeg: "true".into(),
        ..Programs::default()
    };
    let mut process = EncoderProcess::start(
        &programs,
        &spec(Path::new("/videos"), Encoder::Libx264),
        Duration::from_secs(10),
        None,
    )
    .unwrap();
    assert_eq!(process.frame_bytes(), 256 * 144 * 3 / 2);
    match process.write_frame(&[0; 10]) {
        Err(MediaError::Parse(message)) => {
            assert_eq!(message, "an encoder frame holds 55296 bytes, not 10")
        }
        other => panic!("expected a size error, got {other:?}"),
    }
    assert_eq!(process.frames_written(), 0);
    assert_eq!(process.finish().unwrap(), 0);
}

/// A one-second 256x144 source with a test pattern, a sine tone, a subtitle stream and a title;
/// NVENC refuses much smaller frames.
fn source(dir: &Path) -> PathBuf {
    let subtitles = dir.join("lines.srt");
    std::fs::write(&subtitles, "1\n00:00:00,100 --> 00:00:00,900\nHello\n").unwrap();
    let video = dir.join("source.mkv");
    let output = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi"])
        .args([
            "-i",
            "testsrc=size=256x144:rate=24:duration=1",
            "-f",
            "lavfi",
        ])
        .args(["-i", "sine=frequency=440:duration=1", "-i"])
        .arg(&subtitles)
        .args(["-map", "0", "-map", "1", "-map", "2", "-pix_fmt", "yuv420p"])
        .args(["-c:v", "ffv1", "-c:a", "flac", "-c:s", "srt"])
        .args(["-metadata", "title=Pattern", "-y"])
        .arg(&video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    video
}

/// ffprobe's compact answer for `entries` of the streams `select` picks in `video`.
fn probe_entries(video: &Path, select: &str, entries: &str) -> String {
    let output = Run::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            select,
            "-show_entries",
            entries,
        ])
        .args(["-of", "csv=p=0"])
        .arg(video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    output.stdout.trim().to_string()
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_round_trip_keeps_every_frame_and_the_audio_and_drops_the_subtitles() {
    let dir = scratch("round-trip");
    let video = source(&dir);
    let programs = Programs::default();
    let encoder = available_encoder(&programs).unwrap();
    let spec = spec(&dir, encoder);
    let mut frames = FrameStream::open_native(
        &programs,
        &video,
        (256, 144),
        0.0,
        24.0,
        PixelFormat::Yuv420p,
        Decode::Exact,
    )
    .unwrap();
    let mut process =
        EncoderProcess::start(&programs, &spec, Duration::from_secs(120), None).unwrap();
    while let Some(frame) = frames.next_frame().unwrap() {
        process.write_frame(&frame.rgb).unwrap();
    }
    frames.finish().unwrap();
    assert_eq!(process.finish().unwrap(), 24);
    let encoded = timeline(&programs, &spec.output, 0.0, 24.0).unwrap();
    assert_eq!(encoded.len(), 24);
    let codec = probe_entries(&spec.output, "v", "stream=codec_name");
    assert_eq!(
        codec,
        if encoder == Encoder::HevcNvenc {
            "hevc"
        } else {
            "h264"
        }
    );
    assert_eq!(
        probe_entries(&spec.output, "a", "stream=codec_name"),
        "flac"
    );
    assert_eq!(probe_entries(&spec.output, "s", "stream=index"), "");
    assert_eq!(
        probe_entries(&spec.output, "v", "format_tags=title"),
        "Pattern"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_cancel_stops_the_encode_and_says_so() {
    let dir = scratch("cancel");
    source(&dir);
    let programs = Programs::default();
    let flag = Arc::new(AtomicBool::new(false));
    let mut process = EncoderProcess::start(
        &programs,
        &spec(&dir, Encoder::Libx264),
        Duration::from_secs(120),
        Some(flag.clone()),
    )
    .unwrap();
    let frame = vec![128u8; process.frame_bytes()];
    process.write_frame(&frame).unwrap();
    flag.store(true, Ordering::SeqCst);
    let stopped = (0..100_000).any(|_| process.write_frame(&frame).is_err());
    assert!(stopped, "writes must fail once the encoder is killed");
    assert!(matches!(
        process.finish(),
        Err(MediaError::Run(RunError::Cancelled { .. }))
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_failed_encode_reports_ffmpegs_reason() {
    let dir = scratch("missing-source");
    let programs = Programs::default();
    let process = EncoderProcess::start(
        &programs,
        &spec(&dir, Encoder::Libx264),
        Duration::from_secs(60),
        None,
    )
    .unwrap();
    match process.finish() {
        Err(MediaError::Exit { stderr, code, .. }) => {
            assert_ne!(code, 0);
            assert!(stderr.contains("source.mkv"), "{stderr}");
        }
        other => panic!("expected an exit error, got {other:?}"),
    }
    std::fs::remove_dir_all(dir).unwrap();
}

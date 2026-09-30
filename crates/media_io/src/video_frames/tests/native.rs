use std::path::PathBuf;
use std::time::Duration;

use child_process::Run;

use super::*;

#[test]
fn frame_sizes_follow_the_pixel_format() {
    assert_eq!(PixelFormat::Rgb24.frame_bytes((64, 36)).unwrap(), 6912);
    assert_eq!(PixelFormat::Yuv420p.frame_bytes((64, 36)).unwrap(), 3456);
    assert_eq!(
        PixelFormat::Yuv420p10le.frame_bytes((64, 36)).unwrap(),
        6912
    );
    assert_eq!(
        PixelFormat::Yuv420p.frame_bytes((1920, 1080)).unwrap(),
        3_110_400
    );
    assert_eq!(PixelFormat::Rgb24.frame_bytes((3, 5)).unwrap(), 45);
    assert_eq!(
        (
            PixelFormat::Rgb24.name(),
            PixelFormat::Yuv420p.name(),
            PixelFormat::Yuv420p10le.name()
        ),
        ("rgb24", "yuv420p", "yuv420p10le")
    );
    assert!(PixelFormat::Yuv420p10le.is_high_bit_depth());
    assert!(!PixelFormat::Yuv420p.is_high_bit_depth());
}

#[test]
fn an_odd_size_is_refused_for_subsampled_formats() {
    for format in [PixelFormat::Yuv420p, PixelFormat::Yuv420p10le] {
        for size in [(63, 36), (64, 35)] {
            match format.frame_bytes(size) {
                Err(MediaError::Parse(message)) => assert!(message.contains("even"), "{message}"),
                other => panic!("expected an odd-size error, got {other:?}"),
            }
        }
    }
    assert!(PixelFormat::Rgb24.frame_bytes((0, 36)).is_err());
    assert!(PixelFormat::Yuv420p.frame_bytes((0, 0)).is_err());
    assert!(
        PixelFormat::Rgb24
            .frame_bytes((u32::MAX, u32::MAX))
            .is_err()
    );
}

#[test]
fn native_arguments_have_no_scale_and_name_the_format() {
    let args = native_args(
        Path::new("/videos/a.mkv"),
        PixelFormat::Yuv420p10le,
        Decode::Exact,
    );
    let expected: Vec<String> = [
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-noautorotate",
        "-i",
        "/videos/a.mkv",
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "yuv420p10le",
        "-f",
        "rawvideo",
        "pipe:1",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(args, expected);
}

#[test]
fn an_odd_native_size_is_refused_before_anything_runs() {
    let programs = Programs {
        ffprobe: "/nonexistent/ffprobe".into(),
        ffmpeg: "/nonexistent/ffmpeg".into(),
        bundled: false,
    };
    let opened = FrameStream::open_native(
        &programs,
        Path::new("v.mkv"),
        (63, 36),
        0.0,
        24.0,
        PixelFormat::Yuv420p,
        Decode::Exact,
    );
    assert!(matches!(opened, Err(MediaError::Parse(_))));
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("native-frames-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_native_stream_yields_one_raw_frame_per_timeline_entry() {
    let dir = scratch("count");
    let video = dir.join("pattern.mkv");
    let output = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=size=64x36:rate=24:duration=0.5"])
        .args(["-pix_fmt", "yuv420p", "-c:v", "ffv1", "-y"])
        .arg(&video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    let programs = Programs::default();
    for format in [PixelFormat::Yuv420p, PixelFormat::Yuv420p10le] {
        let mut stream = FrameStream::open_native(
            &programs,
            &video,
            (64, 36),
            0.0,
            24.0,
            format,
            Decode::Exact,
        )
        .unwrap();
        assert_eq!(stream.timeline().len(), 12);
        let mut frames = 0;
        while let Some(frame) = stream.next_frame().unwrap() {
            assert_eq!(frame.index, frames);
            assert_eq!(frame.rgb.len(), format.frame_bytes((64, 36)).unwrap());
            frames += 1;
        }
        stream.finish().unwrap();
        assert_eq!(frames, 12);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

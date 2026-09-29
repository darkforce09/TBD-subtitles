use std::path::PathBuf;

use super::*;
use crate::video_frames::{Decode, FrameStream};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("still-frame-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 12-frame 64x36 test pattern at 24 frames per second, losslessly encoded.
fn pattern(dir: &Path) -> PathBuf {
    let video = dir.join("pattern.mkv");
    let output = Run::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
        ])
        .arg("testsrc=size=64x36:rate=24:duration=0.5")
        .args(["-c:v", "ffv1", "-fps_mode", "passthrough"])
        .args(["-avoid_negative_ts", "disabled", "-y"])
        .arg(&video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    video
}

/// Every frame of the pattern as streamed, with the stream's timeline.
fn streamed(video: &Path) -> (Vec<(f64, f64)>, Vec<Vec<u8>>) {
    let programs = Programs::default();
    let mut stream =
        FrameStream::open(&programs, video, (64, 36), 0.0, 24.0, Decode::Exact).unwrap();
    let timeline = stream.timeline().to_vec();
    let mut frames = Vec::new();
    while let Some(frame) = stream.next_frame().unwrap() {
        frames.push(frame.rgb);
    }
    stream.finish().unwrap();
    (timeline, frames)
}

#[test]
fn a_still_is_the_streamed_frame_at_the_same_time() {
    let dir = scratch("match");
    let video = pattern(&dir);
    let (timeline, frames) = streamed(&video);
    assert_eq!(frames.len(), 12);
    assert!(frames[6] != frames[7] && frames[7] != frames[8]);
    let programs = Programs::default();
    for index in [7, 0, 11] {
        let rgb = still(&programs, &video, timeline[index].0, 24.0, (64, 36)).unwrap();
        assert_eq!(rgb.len(), 64 * 36 * 3);
        assert!(rgb == frames[index], "frame {index} differs");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_time_past_the_end_or_a_missing_video_is_an_error() {
    let dir = scratch("missing");
    let video = pattern(&dir);
    let programs = Programs::default();
    match still(&programs, &video, 10.0, 24.0, (64, 36)) {
        Err(MediaError::Parse(message)) => assert_eq!(message, "still frame is incomplete"),
        other => panic!("expected an incomplete still, got {other:?}"),
    }
    let absent = dir.join("absent.mkv");
    assert!(matches!(
        still(&programs, &absent, 0.0, 24.0, (64, 36)),
        Err(MediaError::Exit { .. })
    ));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn invalid_times_rates_and_sizes_are_rejected_before_ffmpeg_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ffprobe: "/nonexistent/ffprobe".into(),
        bundled: false,
    };
    let video = Path::new("/nonexistent/video.mkv");
    for (time_s, fps, size) in [
        (f64::NAN, 24.0, (64, 36)),
        (f64::INFINITY, 24.0, (64, 36)),
        (-0.001, 24.0, (64, 36)),
        (0.0, 0.0, (64, 36)),
        (0.0, -24.0, (64, 36)),
        (0.0, f64::NAN, (64, 36)),
        (0.0, f64::INFINITY, (64, 36)),
        (0.0, 1e-320, (64, 36)),
        (0.0, 24.0, (0, 36)),
        (0.0, 24.0, (64, 0)),
        (0.0, 24.0, (16_384, 16_384)),
        (0.0, 24.0, (u32::MAX, u32::MAX)),
    ] {
        assert!(
            matches!(
                still(&programs, video, time_s, fps, size),
                Err(MediaError::Parse(_))
            ),
            "{time_s} {fps} {size:?}"
        );
    }
}

#[test]
fn the_seek_lands_half_a_frame_early_and_never_before_the_start() {
    let video = Path::new("/videos/a b.mkv");
    let args = still_args(video, 1.0, 24.0, (64, 36));
    let seek = args.iter().position(|arg| arg == "-ss").unwrap();
    assert_eq!(args[seek + 1], "0.979167");
    assert_eq!(args[seek - 2..seek], ["-seek_timestamp", "0"]);
    let input = args.iter().position(|arg| arg == "-i").unwrap();
    assert!(seek < input);
    assert_eq!(args[input + 1], "/videos/a b.mkv");
    assert!(args.windows(2).any(|pair| pair == ["-frames:v", "1"]));
    assert!(args.windows(2).any(|pair| pair == ["-vf", "scale=64:36"]));
    assert_eq!(args.last().map(String::as_str), Some("pipe:1"));
    let first = still_args(video, 0.0, 24.0, (64, 36));
    assert_eq!(first[seek + 1], "0.000000");
    let early = still_args(video, 0.01, 24.0, (64, 36));
    assert_eq!(early[seek + 1], "0.000000");
}

use std::path::PathBuf;

use super::*;
use crate::video_frames::{Decode, FrameStream};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("region-frames-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 12-frame 64x36 4:2:0 test pattern at 24 frames per second, losslessly encoded with a
/// keyframe every 4 frames, so a seek decodes from an earlier keyframe.
fn pattern(dir: &Path) -> PathBuf {
    let video = dir.join("pattern.mkv");
    let output = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=size=64x36:rate=24:duration=0.5"])
        .args(["-pix_fmt", "yuv420p", "-c:v", "ffv1", "-g", "4"])
        .args([
            "-fps_mode",
            "passthrough",
            "-avoid_negative_ts",
            "disabled",
            "-y",
        ])
        .arg(&video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    video
}

/// The rectangle (x, y, width, height) of a 64-pixel-wide rgb24 frame.
fn cropped(frame: &[u8], (x, y, width, height): (u32, u32, u32, u32)) -> Vec<u8> {
    let mut crop = Vec::new();
    for row in y..y + height {
        let start = ((row * 64 + x) * 3) as usize;
        crop.extend_from_slice(&frame[start..start + (width * 3) as usize]);
    }
    crop
}

#[test]
fn the_arguments_seek_half_a_frame_early_and_crop_after_conversion() {
    let args = region_args(Path::new("/videos/a b.mkv"), (5, 3, 20, 11), 1.0, 25.0, 7);
    let expected: Vec<String> = [
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-seek_timestamp",
        "0",
        "-ss",
        "0.980000",
        "-noautorotate",
        "-i",
        "/videos/a b.mkv",
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-vf",
        "scale=iw:ih,format=rgb24,crop=20:11:5:3",
        "-frames:v",
        "7",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "rgb24",
        "-f",
        "rawvideo",
        "pipe:1",
    ]
    .map(String::from)
    .to_vec();
    assert_eq!(args, expected);
    let clamped = region_args(Path::new("v.mkv"), (0, 0, 2, 2), 0.0, 24.0, 1);
    assert_eq!(clamped[7], "0.000000", "the seek clamps at the start");
}

#[test]
fn the_deadline_grows_with_the_frame_count() {
    assert_eq!(deadline(1), Duration::from_secs_f64(120.25));
    assert_eq!(deadline(2400), Duration::from_secs(720));
}

#[test]
fn an_invalid_request_is_refused_before_anything_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let video = Path::new("v.mkv");
    for (crop, time, fps, count) in [
        ((0, 0, 0, 4), 0.0, 24.0, 1),
        ((0, 0, 4, 4), -1.0, 24.0, 1),
        ((0, 0, 4, 4), f64::NAN, 24.0, 1),
        ((0, 0, 4, 4), 0.0, 0.0, 1),
        ((0, 0, 4, 4), 0.0, 24.0, 0),
    ] {
        assert!(matches!(
            RegionStream::open(&programs, video, crop, time, fps, count),
            Err(MediaError::Parse(_))
        ));
    }
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_region_run_is_the_same_rectangle_of_the_streamed_frames() {
    let dir = scratch("match");
    let video = pattern(&dir);
    let programs = Programs::default();
    let mut stream =
        FrameStream::open(&programs, &video, (64, 36), 0.0, 24.0, Decode::Exact).unwrap();
    let timeline = stream.timeline().to_vec();
    let mut frames = Vec::new();
    while let Some(frame) = stream.next_frame().unwrap() {
        frames.push(frame.rgb);
    }
    stream.finish().unwrap();
    assert_eq!(frames.len(), 12);
    let crop = (5, 3, 21, 11);
    for (first, count) in [(5usize, 4u64), (0, 12), (11, 1)] {
        let mut region =
            RegionStream::open(&programs, &video, crop, timeline[first].0, 24.0, count).unwrap();
        let mut index = first;
        while let Some(rgb) = region.next_frame().unwrap() {
            assert_eq!(rgb.len(), 21 * 11 * 3);
            assert!(
                rgb == cropped(&frames[index], crop),
                "frame {index} differs"
            );
            index += 1;
        }
        region.finish().unwrap();
        assert_eq!(index, first + count as usize);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_run_past_the_end_or_wider_than_the_frame_is_an_error() {
    let dir = scratch("short");
    let video = pattern(&dir);
    let programs = Programs::default();
    let time_s = 10.0 / 24.0;
    let mut region = RegionStream::open(&programs, &video, (0, 0, 8, 8), time_s, 24.0, 5).unwrap();
    let mut read = 0;
    while region.next_frame().unwrap().is_some() {
        read += 1;
    }
    assert_eq!(read, 2);
    match region.finish() {
        Err(MediaError::Parse(message)) => {
            assert_eq!(message, "region decode ended after 2 of 5 frames")
        }
        other => panic!("expected a short run, got {other:?}"),
    }
    let mut outside = RegionStream::open(&programs, &video, (0, 0, 80, 8), 0.0, 24.0, 1).unwrap();
    assert!(outside.next_frame().unwrap().is_none());
    assert!(matches!(outside.finish(), Err(MediaError::Exit { .. })));
    std::fs::remove_dir_all(dir).unwrap();
}

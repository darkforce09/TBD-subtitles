use std::process::Command;

use media_io::video_frames::{Decode, FrameStream};

use super::*;

fn stream(width: u32, height: u32) -> VideoStream {
    VideoStream {
        index: 0,
        codec: "ffv1".into(),
        width,
        height,
        frame_rate_num: 24,
        frame_rate_den: 1,
        start_time_s: 0.0,
        pix_fmt: None,
        color_primaries: None,
        color_transfer: None,
        color_space: None,
        color_range: None,
        bit_rate: None,
    }
}

/// Regions over a scripted timeline whose FFmpeg cannot run, so only validation is exercised.
fn scripted(timeline: Vec<(f64, f64)>) -> FfmpegRegions {
    FfmpegRegions {
        programs: Programs {
            ffmpeg: "/nonexistent/ffmpeg".into(),
            ffprobe: "/nonexistent/ffprobe".into(),
            bundled: false,
        },
        video: PathBuf::from("/videos/v.mkv"),
        fps: 24.0,
        timeline,
        size: (64, 36),
    }
}

fn rect(x: u32, y: u32, width: u32, height: u32) -> PixelRect {
    PixelRect {
        x,
        y,
        width,
        height,
    }
}

fn ten_frames() -> Vec<(f64, f64)> {
    (0..10)
        .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
        .collect()
}

#[test]
fn a_span_outside_the_timeline_or_a_rectangle_outside_the_frame_is_refused() {
    let mut regions = scripted(ten_frames());
    let mut visited = 0;
    let mut visit = |_: u64, _: RgbImage| -> TextResult<()> {
        visited += 1;
        Ok(())
    };
    let whole = rect(0, 0, 64, 36);
    for (area, first, last) in [
        (whole, 5, 4),
        (whole, 0, 10),
        (rect(60, 0, 5, 4), 0, 0),
        (rect(0, 33, 4, 4), 0, 0),
        (rect(0, 0, 0, 4), 0, 0),
        (rect(u32::MAX, 0, 2, 2), 0, 0),
    ] {
        let refused = regions.frames(area, first, last, &mut visit);
        assert!(refused.is_err(), "{area:?} {first}..={last}");
    }
    assert_eq!(visited, 0);
    assert!(inside(whole, (64, 36)));
    assert_eq!(span_start(9, 9, 10).unwrap(), 9);
}

#[test]
fn the_seek_margin_follows_the_gap_to_the_previous_frame() {
    let regions = scripted(vec![(0.0, 0.05), (0.05, 0.15), (0.15, 0.2), (0.2, 0.2)]);
    assert_eq!(regions.seek_rate(0), 24.0);
    assert!((regions.seek_rate(1) - 20.0).abs() < 1e-9);
    assert!((regions.seek_rate(2) - 10.0).abs() < 1e-9);
    assert!((regions.seek_rate(3) - 20.0).abs() < 1e-9);
    let clipped = scripted(vec![(0.0, 0.0), (0.0, 0.04)]);
    assert_eq!(clipped.seek_rate(1), 24.0, "a zero gap falls back");
}

#[test]
fn an_unreported_frame_rate_falls_back_to_24() {
    let mut unknown = stream(64, 36);
    unknown.frame_rate_den = 0;
    assert_eq!(frame_rate(&unknown), 24.0);
    let mut ntsc = stream(64, 36);
    (ntsc.frame_rate_num, ntsc.frame_rate_den) = (24000, 1001);
    assert!((frame_rate(&ntsc) - 23.976).abs() < 0.001);
}

#[test]
#[ignore = "needs FFmpeg"]
fn every_crop_of_a_span_is_the_same_rectangle_of_the_source_frame() {
    let dir = std::env::temp_dir().join(format!("region-source-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let video = dir.join("pattern.mkv");
    let made = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=size=64x36:rate=24:duration=0.5"])
        .args(["-pix_fmt", "yuv420p", "-c:v", "ffv1", "-g", "5", "-y"])
        .arg(&video)
        .status()
        .unwrap();
    assert!(made.success());
    let programs = Programs::default();
    let mut frames =
        FrameStream::open(&programs, &video, (64, 36), 0.0, 24.0, Decode::Exact).unwrap();
    let mut whole = Vec::new();
    while let Some(frame) = frames.next_frame().unwrap() {
        whole.push(RgbImage::from_raw(64, 36, frame.rgb).unwrap());
    }
    frames.finish().unwrap();
    let mut regions = FfmpegRegions::open(&programs, &video, &stream(64, 36)).unwrap();
    assert_eq!(regions.timeline().len(), 12);
    assert_eq!(regions.frame_size(), (64, 36));
    let area = rect(7, 5, 19, 13);
    let mut seen = Vec::new();
    regions
        .frames(area, 3, 8, &mut |index, crop| {
            let expected = image::imageops::crop_imm(&whole[index as usize], 7, 5, 19, 13);
            assert!(crop == expected.to_image(), "frame {index} differs");
            seen.push(index);
            Ok(())
        })
        .unwrap();
    assert_eq!(seen, vec![3, 4, 5, 6, 7, 8]);
    let stopped = regions.frames(area, 0, 11, &mut |index, _| {
        if index == 2 {
            Err("stop".into())
        } else {
            Ok(())
        }
    });
    assert!(stopped.is_err(), "a visitor's error stops the span");
    std::fs::remove_dir_all(dir).unwrap();
}

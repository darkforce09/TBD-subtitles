use std::path::PathBuf;
use std::time::Duration;

use child_process::Run;

use super::*;
use crate::video_frames::{Decode, FrameStream, timeline};
use crate::yuv::{Matrix, Range};

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

/// The colour FFmpeg's rgb24 conversion gives an untagged standard-definition stream.
fn untagged() -> Coefficients {
    Coefficients::new(Matrix::Bt601, Range::Limited)
}

fn request(region: (u32, u32, u32, u32), first: u64, count: u64) -> RegionRequest {
    RegionRequest {
        frame_size: (64, 36),
        fps: 24.0,
        region,
        first,
        count,
        colour: untagged(),
    }
}

#[test]
fn an_odd_region_is_cropped_on_even_edges_and_trimmed_back() {
    let odd = RegionCrop::around((7, 5, 19, 13), (64, 36)).unwrap();
    assert_eq!(odd.decoded, (6, 4, 20, 14));
    assert_eq!(
        odd.trim,
        Rect {
            x: 1,
            y: 1,
            width: 19,
            height: 13
        }
    );
    let even = RegionCrop::around((8, 4, 20, 10), (64, 36)).unwrap();
    assert_eq!(even.decoded, (8, 4, 20, 10));
    assert_eq!((even.trim.x, even.trim.y), (0, 0));
    let corner = RegionCrop::around((63, 35, 1, 1), (64, 36)).unwrap();
    assert_eq!(corner.decoded, (62, 34, 2, 2));
    assert_eq!((corner.trim.x, corner.trim.y), (1, 1));
    let odd_frame = RegionCrop::around((60, 0, 5, 4), (65, 36)).unwrap();
    assert_eq!(
        odd_frame.decoded,
        (60, 0, 5, 4),
        "clamped to an odd frame's edge"
    );
    for region in [
        (0, 0, 0, 4),
        (0, 0, 4, 0),
        (60, 0, 5, 4),
        (0, 33, 4, 4),
        (u32::MAX, 0, 2, 2),
    ] {
        assert!(RegionCrop::around(region, (64, 36)).is_err(), "{region:?}");
    }
}

#[test]
fn an_invalid_request_is_refused_before_anything_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let video = Path::new("v.mkv");
    let timeline: Arc<[(f64, f64)]> = (0..12)
        .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
        .collect::<Vec<_>>()
        .into();
    let odd_edge = RegionRequest {
        frame_size: (65, 36),
        ..request((60, 0, 5, 4), 0, 1)
    };
    let no_rate = RegionRequest {
        fps: 0.0,
        ..request((0, 0, 4, 4), 0, 1)
    };
    for bad in [
        request((0, 0, 0, 4), 0, 1),
        request((0, 0, 80, 8), 0, 1),
        request((0, 0, 4, 4), 0, 0),
        request((0, 0, 4, 4), 10, 5),
        request((0, 0, 4, 4), 12, 1),
        odd_edge,
        no_rate,
    ] {
        assert!(
            matches!(
                RegionStream::open(&programs, video, timeline.clone(), bad),
                Err(MediaError::Parse(_))
            ),
            "{bad:?}"
        );
    }
}

#[test]
#[ignore = "needs FFmpeg"]
fn yuv_region_crops_match_the_rgb24_route_within_two_levels() {
    let dir = scratch("match");
    let video = pattern(&dir);
    let programs = Programs::default();
    let mut stream =
        FrameStream::open(&programs, &video, (64, 36), 0.0, 24.0, Decode::Exact).unwrap();
    let timeline: Arc<[(f64, f64)]> = stream.timeline().to_vec().into();
    let mut frames = Vec::new();
    while let Some(frame) = stream.next_frame().unwrap() {
        frames.push(frame.rgb);
    }
    stream.finish().unwrap();
    assert_eq!(frames.len(), 12);
    let mut levels = [0usize; 256];
    for (region, first, count) in [
        ((7, 5, 19, 13), 5u64, 4u64),
        ((0, 0, 64, 36), 0, 12),
        ((5, 3, 21, 11), 11, 1),
        ((63, 35, 1, 1), 2, 3),
    ] {
        let mut crops = RegionStream::open(
            &programs,
            &video,
            timeline.clone(),
            request(region, first, count),
        )
        .unwrap();
        let mut index = first;
        while let Some(crop) = crops.next_frame().unwrap() {
            assert_eq!(crop.index, index);
            assert_eq!(crop.rgb.len(), (region.2 * region.3 * 3) as usize);
            let reference = cropped(&frames[index as usize], region);
            for (ours, theirs) in crop.rgb.iter().zip(&reference) {
                levels[usize::from(ours.abs_diff(*theirs))] += 1;
            }
            index += 1;
        }
        crops.finish().unwrap();
        assert_eq!(index, first + count);
    }
    let samples: usize = levels.iter().sum();
    let worst = levels.iter().rposition(|n| *n > 0).unwrap_or(0);
    let beyond_two: usize = levels[3..].iter().sum();
    println!("differences by level: {:?}; max {worst}", &levels[..=worst]);
    assert!(worst <= 3, "max difference {worst}");
    assert!(
        beyond_two * 100 <= samples,
        "{beyond_two} of {samples} beyond two levels"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn crops_run_ahead_through_the_queue_in_index_order() {
    let dir = scratch("queue");
    let video = pattern(&dir);
    let programs = Programs::default();
    let timeline: Arc<[(f64, f64)]> = timeline(&programs, &video, 0.0, 24.0).unwrap().into();
    let mut whole = Vec::new();
    let mut crops = RegionStream::open(
        &programs,
        &video,
        timeline.clone(),
        request((7, 5, 19, 13), 3, 6),
    )
    .unwrap();
    while let Some(crop) = crops.next_frame().unwrap() {
        whole.push(crop);
    }
    crops.finish().unwrap();
    let mut queue = RegionStream::open(
        &programs,
        &video,
        timeline.clone(),
        request((7, 5, 19, 13), 3, 6),
    )
    .unwrap()
    .spawn();
    let mut read = 0;
    while let Some(crop) = queue.recv().unwrap() {
        assert_eq!(crop.index, whole[read].index);
        assert!(crop.rgb == whole[read].rgb);
        read += 1;
    }
    queue
        .finish(|| MediaError::Parse("the decode thread panicked".into()))
        .unwrap();
    assert_eq!(read, 6);
    let mut early = RegionStream::open(&programs, &video, timeline, request((0, 0, 8, 8), 0, 12))
        .unwrap()
        .spawn();
    assert!(early.recv().unwrap().is_some());
    early
        .finish(|| MediaError::Parse("the decode thread panicked".into()))
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

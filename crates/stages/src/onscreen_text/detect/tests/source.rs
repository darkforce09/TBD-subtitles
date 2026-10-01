use std::process::Command;

use super::*;
use media_io::video_frames::{Decode, FrameStream, PixelFormat};
use media_io::yuv::ChromaLayout;

fn stream(width: u32, height: u32) -> VideoStream {
    VideoStream {
        index: 0,
        codec: "ffv1".into(),
        width,
        height,
        frame_rate_num: 24,
        frame_rate_den: 1,
        start_time_s: 0.0,
        ..VideoStream::default()
    }
}

#[test]
#[ignore = "needs FFmpeg"]
fn the_ffmpeg_source_hands_on_every_yuv_frame_in_order_and_stills_by_index() {
    let dir = std::env::temp_dir().join(format!("detect-source-{}", std::process::id()));
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
    let mut expected = FrameStream::open_native(
        &programs,
        &video,
        (64, 36),
        0.0,
        24.0,
        PixelFormat::Yuv420p,
        Decode::Exact,
    )
    .unwrap();
    let mut source = FfmpegSource::open(&programs, &video, &stream(64, 36), false).unwrap();
    assert_eq!(source.frame_size(), (64, 36));
    assert_eq!(source.timeline(), expected.timeline());
    let mut count = 0;
    while let Some(frame) = source.next_frame().unwrap() {
        let reference = expected.next_frame().unwrap().unwrap();
        assert_eq!(frame.index, count);
        assert_eq!(
            (frame.time_s, frame.end_s),
            (reference.time_s, reference.end_s)
        );
        assert_eq!(
            (frame.width, frame.height, frame.layout),
            (64, 36, ChromaLayout::Planar)
        );
        assert_eq!(&frame.data[..], &reference.rgb[..], "frame {count}");
        assert!(frame.picture().is_some());
        count += 1;
    }
    assert_eq!(count, 12);
    source.finish().unwrap();
    expected.finish().unwrap();
    let mut rgb = FrameStream::open(&programs, &video, (64, 36), 0.0, 24.0, Decode::Exact).unwrap();
    let mut whole = Vec::new();
    while let Some(frame) = rgb.next_frame().unwrap() {
        whole.push(frame.rgb);
    }
    rgb.finish().unwrap();
    let stills = source.stills(&[7, 3]).unwrap();
    assert_eq!(stills.len(), 2);
    assert_eq!(stills[0].as_raw(), &whole[7]);
    assert_eq!(stills[1].as_raw(), &whole[3]);
    assert!(
        source.stills(&[12]).is_err(),
        "frame 12 is past the timeline"
    );
    std::fs::remove_dir_all(dir).unwrap();
}

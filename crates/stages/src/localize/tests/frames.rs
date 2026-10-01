use std::path::PathBuf;
use std::process::Command;

use media_io::video_frames::timeline;

use super::*;

#[test]
fn a_native_queue_holds_seconds_of_frames_within_its_byte_bound() {
    let eight_bit_1080p = PixelFormat::Yuv420p.frame_bytes((1920, 1080)).unwrap();
    assert_eq!(queue_depth(23.976, eight_bit_1080p), 96);
    let ten_bit_4k = PixelFormat::Yuv420p10le.frame_bytes((3840, 2160)).unwrap();
    assert_eq!(queue_depth(60.0, ten_bit_4k), QUEUE_MAX_BYTES / ten_bit_4k);
    assert_eq!(queue_depth(0.1, 100), 2, "at least two frames");
}

/// A two-second 64x48 test pattern at 24 fps in `pix_fmt`.
fn clip(name: &str, pix_fmt: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("tbd-localize-frames-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let video = dir.join("clip.mkv");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
        ])
        .args(["testsrc=size=64x48:rate=24:duration=2", "-c:v", "libx264"])
        .args(["-pix_fmt", pix_fmt])
        .arg(&video)
        .status()
        .unwrap();
    assert!(status.success());
    video
}

/// Every frame index `decoder` hands out, then its finish.
fn drain(mut decoder: Decoder) -> (Vec<u64>, Result<(), MediaError>) {
    let mut indices = Vec::new();
    while let Some(frame) = decoder.recv().unwrap() {
        indices.push(frame.index);
    }
    (indices, decoder.finish())
}

#[test]
#[ignore = "needs FFmpeg"]
fn an_eight_bit_decoder_starts_at_any_frame_and_stops_after_its_count() {
    let programs = Programs::default();
    let video = clip("yuv", "yuv420p");
    let frames: Arc<[(f64, f64)]> = timeline(&programs, &video, 0.0, 24.0).unwrap().into();
    let decoder = Decoder::yuv(
        &programs,
        &video,
        (64, 48),
        24.0,
        frames.clone(),
        13,
        Some(7),
    )
    .unwrap();
    let (indices, finished) = drain(decoder);
    assert_eq!(indices, (13..20).collect::<Vec<u64>>());
    assert!(finished.is_ok(), "{finished:?}");
    let decoder = Decoder::yuv(&programs, &video, (64, 48), 24.0, frames, 0, None).unwrap();
    let (indices, finished) = drain(decoder);
    assert_eq!(indices.len(), 48);
    assert!(finished.is_ok(), "{finished:?}");
    std::fs::remove_dir_all(video.parent().unwrap()).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_ten_bit_decoder_hands_out_only_its_ranges() {
    let programs = Programs::default();
    let video = clip("native", "yuv420p10le");
    let format = PixelFormat::Yuv420p10le;
    let ranges = vec![(3, 5), (20, 22)];
    let decoder = Decoder::native(&programs, &video, (64, 48), 0.0, 24.0, format, ranges).unwrap();
    let (indices, finished) = drain(decoder);
    assert_eq!(indices, vec![3, 4, 5, 20, 21, 22]);
    assert!(finished.is_ok(), "{finished:?}");
    let decoder = Decoder::native(
        &programs,
        &video,
        (64, 48),
        0.0,
        24.0,
        format,
        vec![(40, 47)],
    )
    .unwrap();
    let (indices, finished) = drain(decoder);
    assert_eq!(indices, (40..48).collect::<Vec<u64>>());
    assert!(
        finished.is_ok(),
        "read to the end, the decoder is reaped: {finished:?}"
    );
    let refused =
        Decoder::native(&programs, &video, (64, 48), 0.0, 24.0, format, vec![(5, 3)]).err();
    assert!(refused.is_some(), "a backwards range is refused");
    std::fs::remove_dir_all(video.parent().unwrap()).unwrap();
}

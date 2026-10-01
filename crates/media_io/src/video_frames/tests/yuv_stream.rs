use std::path::PathBuf;

use super::*;
use crate::video_frames::{Decode, FrameStream};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("yuv-stream-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A 12-frame 64x36 test pattern at 24 frames per second in `pix_fmt`, losslessly encoded with a
/// keyframe every 4 frames, so a seek decodes from an earlier keyframe.
fn pattern(dir: &Path, pix_fmt: &str) -> PathBuf {
    let video = dir.join(format!("pattern-{pix_fmt}.mkv"));
    let output = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=size=64x36:rate=24:duration=0.5"])
        .args(["-pix_fmt", pix_fmt, "-c:v", "ffv1", "-g", "4"])
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

/// Every frame of `video` as yuv420p bytes through `FrameStream::open_native`.
fn native_frames(video: &Path) -> Vec<Vec<u8>> {
    let mut stream = FrameStream::open_native(
        &Programs::default(),
        video,
        (64, 36),
        0.0,
        24.0,
        PixelFormat::Yuv420p,
        Decode::Exact,
    )
    .unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = stream.next_frame().unwrap() {
        frames.push(frame.rgb);
    }
    stream.finish().unwrap();
    frames
}

fn strings(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| arg.to_string()).collect()
}

#[test]
fn the_cpu_route_decodes_every_frame_exactly_as_8_bit_yuv420p() {
    let args = yuv_args(Path::new("/videos/a b.mkv"), None, YuvOptions::default());
    let expected = strings(&[
        "-nostdin",
        "-hide_banner",
        "-v",
        "error",
        "-noautorotate",
        "-i",
        "/videos/a b.mkv",
        "-map",
        "0:v:0",
        "-an",
        "-sn",
        "-dn",
        "-fps_mode",
        "passthrough",
        "-pix_fmt",
        "yuv420p",
        "-f",
        "rawvideo",
        "pipe:1",
    ]);
    assert_eq!(args, expected);
    assert!(!args.iter().any(|arg| arg == "-skip_loop_filter"));
}

#[test]
fn a_start_seeks_before_the_input_and_a_count_and_crop_follow_the_map() {
    let options = YuvOptions {
        first: 25,
        count: Some(7),
        hardware: false,
        crop: Some((4, 2, 20, 10)),
    };
    let args = yuv_args(Path::new("v.mkv"), Some(0.98), options).join(" ");
    assert_eq!(
        args,
        "-nostdin -hide_banner -v error -seek_timestamp 0 -ss 0.980000 -noautorotate -i v.mkv \
         -map 0:v:0 -an -sn -dn -fps_mode passthrough -vf crop=20:10:4:2 -frames:v 7 \
         -pix_fmt yuv420p -f rawvideo pipe:1"
    );
}

#[test]
fn the_nvdec_route_decodes_on_the_gpu_and_downloads_nv12() {
    let whole = YuvOptions {
        hardware: true,
        ..YuvOptions::default()
    };
    let args = yuv_args(Path::new("/media/ep.mkv"), None, whole).join(" ");
    assert_eq!(
        args,
        "-nostdin -hide_banner -v error -hwaccel cuda -hwaccel_output_format cuda -noautorotate \
         -i /media/ep.mkv -map 0:v:0 -an -sn -dn -fps_mode passthrough \
         -vf hwdownload,format=nv12 -pix_fmt nv12 -f rawvideo pipe:1"
    );
    let cropped = YuvOptions {
        crop: Some((2, 2, 4, 4)),
        ..whole
    };
    let args = yuv_args(Path::new("v.mkv"), Some(1.0), cropped).join(" ");
    assert!(args.starts_with(
        "-nostdin -hide_banner -v error -hwaccel cuda -hwaccel_output_format cuda \
         -seek_timestamp 0 -ss 1.000000 -noautorotate -i v.mkv"
    ));
    assert!(args.contains("-vf hwdownload,format=nv12,crop=4:4:2:2 -pix_fmt nv12"));
}

#[test]
fn nvdec_suits_only_8_bit_4_2_0_sources() {
    let mut stream = VideoStream {
        index: 0,
        codec: "h264".into(),
        width: 1920,
        height: 1080,
        frame_rate_num: 24000,
        frame_rate_den: 1001,
        start_time_s: 0.0,
        pix_fmt: Some("yuv420p".into()),
        color_primaries: None,
        color_transfer: None,
        color_space: None,
        color_range: None,
        bit_rate: None,
    };
    assert!(hardware_suits(&stream));
    stream.pix_fmt = Some("yuv420p10le".into());
    assert!(!hardware_suits(&stream));
    stream.pix_fmt = None;
    assert!(!hardware_suits(&stream));
}

#[test]
fn the_seek_lands_half_the_gap_to_the_previous_frame_early() {
    let timeline = [
        (0.0, 0.05),
        (0.05, 0.15),
        (0.15, 0.2),
        (0.2, 0.2),
        (0.2, 0.3),
    ];
    assert_eq!(
        seek_time(&timeline, 0, 24.0),
        None,
        "the first frame needs no seek"
    );
    assert!((seek_time(&timeline, 1, 24.0).unwrap() - 0.025).abs() < 1e-9);
    assert!((seek_time(&timeline, 2, 24.0).unwrap() - 0.1).abs() < 1e-9);
    let fallback = seek_time(&timeline, 4, 25.0).unwrap();
    assert!(
        (fallback - 0.18).abs() < 1e-9,
        "a zero gap falls back to 1 / fps"
    );
    assert_eq!(
        seek_time(&[(0.0, 0.01), (0.01, 0.02)], 1, 24.0),
        Some(0.005)
    );
}

#[test]
fn a_span_off_the_timeline_or_an_uneven_crop_is_refused() {
    let timeline = [(0.0, 1.0); 10];
    let span = |first, count| YuvOptions {
        first,
        count,
        ..YuvOptions::default()
    };
    assert_eq!(span_end(&timeline, span(0, None)).unwrap(), 10);
    assert_eq!(span_end(&timeline, span(3, Some(7))).unwrap(), 10);
    assert_eq!(span_end(&timeline, span(9, None)).unwrap(), 10);
    for (first, count) in [(3, Some(8)), (10, None), (0, Some(0)), (u64::MAX, Some(2))] {
        assert!(
            span_end(&timeline, span(first, count)).is_err(),
            "{first} {count:?}"
        );
    }
    assert_eq!(output_size((64, 36), None).unwrap(), (64, 36));
    assert_eq!(output_size((64, 36), Some((2, 4, 6, 8))).unwrap(), (6, 8));
    for crop in [
        (1, 0, 4, 4),
        (0, 0, 3, 4),
        (0, 0, 0, 4),
        (62, 0, 4, 4),
        (0, 34, 2, 4),
    ] {
        assert!(output_size((64, 36), Some(crop)).is_err(), "{crop:?}");
    }
}

#[test]
fn an_invalid_request_is_refused_before_anything_runs() {
    let programs = Programs {
        ffmpeg: "/nonexistent/ffmpeg".into(),
        ..Programs::default()
    };
    let timeline: Arc<[(f64, f64)]> = vec![(0.0, 0.04); 4].into();
    let video = Path::new("v.mkv");
    let refused = |size, fps, options| {
        matches!(
            YuvStream::over(&programs, video, size, fps, timeline.clone(), options),
            Err(MediaError::Parse(_))
        )
    };
    let whole = YuvOptions::default();
    assert!(refused((63, 36), 24.0, whole), "an odd frame");
    assert!(refused((64, 36), 0.0, whole), "no frame rate");
    let past = YuvOptions {
        first: 2,
        count: Some(3),
        ..whole
    };
    assert!(refused((64, 36), 24.0, past), "past the timeline");
}

#[test]
fn the_queue_holds_four_seconds_within_its_byte_bound() {
    assert_eq!(queue_depth(24.0, 3_110_400), 96);
    assert_eq!(queue_depth(23.976, 3_110_400), 96);
    assert_eq!(
        queue_depth(60.0, 12_441_600),
        43,
        "4K frames are bounded by bytes"
    );
    assert_eq!(queue_depth(0.1, 100), 2, "at least two");
    assert_eq!(queue_depth(24.0, usize::MAX), 2);
}

#[test]
fn the_deadline_grows_with_the_frame_count() {
    assert_eq!(deadline(Some(1)), Duration::from_secs_f64(120.25));
    assert_eq!(deadline(Some(2400)), Duration::from_secs(720));
    assert_eq!(deadline(None), Duration::from_secs(24 * 3600));
}

#[test]
#[ignore = "needs FFmpeg"]
fn every_frame_matches_the_native_stream_and_its_timeline_entry() {
    let dir = scratch("agree");
    let video = pattern(&dir, "yuv420p");
    let expected = native_frames(&video);
    assert_eq!(expected.len(), 12);
    let programs = Programs::default();
    let mut stream = YuvStream::open(
        &programs,
        &video,
        (64, 36),
        0.0,
        24.0,
        YuvOptions::default(),
    )
    .unwrap();
    let timeline = stream.timeline().clone();
    assert_eq!(timeline.len(), 12);
    assert_eq!(stream.frame_size(), (64, 36));
    let mut read = 0;
    while let Some(frame) = stream.next_frame().unwrap() {
        let index = frame.index as usize;
        assert_eq!(index, read);
        assert_eq!((frame.time_s, frame.end_s), timeline[index]);
        assert_eq!((frame.width, frame.height), (64, 36));
        assert_eq!(frame.layout, ChromaLayout::Planar);
        assert!(frame.picture().is_some());
        assert!(*frame.data == expected[index][..], "frame {index} differs");
        read += 1;
    }
    assert_eq!(read, 12);
    stream.finish().unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_start_at_any_index_yields_exactly_that_frame_first() {
    let dir = scratch("start");
    let video = pattern(&dir, "yuv420p");
    let expected = native_frames(&video);
    let programs = Programs::default();
    let timeline: Arc<[(f64, f64)]> = timeline(&programs, &video, 0.0, 24.0).unwrap().into();
    for (first, count) in [(5u64, Some(4u64)), (1, Some(1)), (4, None), (11, None)] {
        let options = YuvOptions {
            first,
            count,
            ..YuvOptions::default()
        };
        let mut stream =
            YuvStream::over(&programs, &video, (64, 36), 24.0, timeline.clone(), options).unwrap();
        let mut index = first as usize;
        while let Some(frame) = stream.next_frame().unwrap() {
            assert_eq!(frame.index as usize, index);
            assert_eq!(frame.time_s, timeline[index].0);
            assert!(
                *frame.data == expected[index][..],
                "frame {index} from {first}"
            );
            index += 1;
        }
        stream.finish().unwrap();
        let end = count.map_or(12, |count| (first + count) as usize);
        assert_eq!(index, end, "from {first}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn frames_reuse_their_buffers_and_run_ahead_through_the_queue() {
    let dir = scratch("pool");
    let video = pattern(&dir, "yuv420p");
    let programs = Programs::default();
    let mut stream = YuvStream::open(
        &programs,
        &video,
        (64, 36),
        0.0,
        24.0,
        YuvOptions::default(),
    )
    .unwrap();
    let first = stream.next_frame().unwrap().unwrap();
    let address = first.data.as_ptr();
    drop(first);
    let second = stream.next_frame().unwrap().unwrap();
    assert_eq!(
        second.data.as_ptr(),
        address,
        "a returned buffer is read into again"
    );
    drop(second);
    assert_eq!(stream.pool.idle(), 1);
    let pool = stream.pool.clone();
    let mut queue = stream.spawn();
    let mut indices = Vec::new();
    while let Some(frame) = queue.recv().unwrap() {
        indices.push(frame.index);
    }
    queue
        .finish(|| MediaError::Parse("the decode thread panicked".into()))
        .unwrap();
    assert_eq!(indices, (2..12).collect::<Vec<_>>());
    assert!(pool.idle() >= 1, "the queue's frames went back to the pool");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_10_bit_source_streams_as_8_bit_and_a_reader_may_stop_early() {
    let dir = scratch("deep");
    let video = pattern(&dir, "yuv420p10le");
    let expected = native_frames(&video);
    let programs = Programs::default();
    let mut queue = YuvStream::open(
        &programs,
        &video,
        (64, 36),
        0.0,
        24.0,
        YuvOptions::default(),
    )
    .unwrap()
    .spawn();
    let frame = queue.recv().unwrap().unwrap();
    assert_eq!(frame.data.len(), 64 * 36 * 3 / 2);
    assert!(*frame.data == expected[0][..]);
    drop(frame);
    queue
        .finish(|| MediaError::Parse("the decode thread panicked".into()))
        .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
#[ignore = "needs FFmpeg"]
fn a_decoder_that_fails_reports_its_own_exit() {
    let dir = scratch("fail");
    let video = dir.join("missing.mkv");
    let programs = Programs::default();
    let timeline: Arc<[(f64, f64)]> = vec![(0.0, 0.04); 2].into();
    let mut stream = YuvStream::over(
        &programs,
        &video,
        (64, 36),
        24.0,
        timeline,
        YuvOptions::default(),
    )
    .unwrap();
    assert!(matches!(stream.next_frame(), Err(MediaError::Exit { .. })));
    std::fs::remove_dir_all(dir).unwrap();
}

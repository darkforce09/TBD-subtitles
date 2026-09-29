use std::io::Cursor;

use super::*;

#[test]
fn presentation_timing_is_not_replaced_by_an_average_frame_rate() {
    assert_eq!(
        parse_timing(
            "best_effort_timestamp_time=1.250|duration_time=0.080\n",
            0.04
        ),
        Some((1.25, 0.08))
    );
    assert_eq!(
        parse_timing(
            "best_effort_timestamp_time=1.330|duration_time=0.040\n",
            0.04
        ),
        Some((1.33, 0.04))
    );
    assert_eq!(
        parse_timing(
            "side_data_type=H.26[45] User Data Unregistered SEI message\n",
            0.04
        ),
        None
    );
    assert_eq!(parse_timing("best_effort_timestamp_time=NaN", 0.04), None);
}

fn close(found: f64, expected: f64) {
    assert!(
        (found - expected).abs() < 0.000_001,
        "{found} vs {expected}"
    );
}

#[test]
fn container_origin_preserves_a_later_video_start() {
    let origin = parse_origin("5.000000\n").expect("container start");
    let (start, end) = frame_interval((5.75, 0.04), Some((5.83, 0.04)), origin).unwrap();
    close(start, 0.75);
    close(end, 0.83);
    let (start, end) = frame_interval((-0.75, 0.04), None, -2.0).unwrap();
    close(start, 1.25);
    close(end, 1.29);
}

#[test]
fn unavailable_container_timing_uses_the_supplied_origin() {
    for text in ["", "N/A\n", "NaN", "inf", "-inf", "0.0\n1.0"] {
        assert!(parse_origin(text).is_none());
        let origin = parse_origin(text).unwrap_or(4.0);
        let (start, end) = frame_interval((4.5, 0.04), None, origin).unwrap();
        close(start, 0.5);
        close(end, 0.54);
    }
    assert_eq!(parse_origin("0.0"), Some(0.0));
}

#[test]
fn next_presentation_timestamp_overrides_nominal_duration_for_vfr() {
    let mut reader = Cursor::new(concat!(
        "best_effort_timestamp_time=2.000|duration_time=0.040\n",
        "side_data_type=frame metadata\n",
        "best_effort_timestamp_time=2.120|duration_time=0.040\n",
        "best_effort_timestamp_time=2.160|duration_time=N/A\n",
    ));
    let first = read_timing(&mut reader, 0.04).unwrap().unwrap();
    let second = read_timing(&mut reader, 0.04).unwrap().unwrap();
    let (start, end) = frame_interval(first, Some(second), 2.0).unwrap();
    close(start, 0.0);
    close(end, 0.12);
    let last = read_timing(&mut reader, 0.04).unwrap().unwrap();
    let (start, end) = frame_interval(second, Some(last), 2.0).unwrap();
    close(start, 0.12);
    close(end, 0.16);
    let after_last = read_timing(&mut reader, 0.04).unwrap();
    assert_eq!(after_last, None);
    let (start, end) = frame_interval(last, after_last, 2.0).unwrap();
    close(start, 0.16);
    close(end, 0.2);
}

#[test]
fn the_final_frame_keeps_its_reported_duration_and_invalid_duration_falls_back() {
    let last = parse_timing("best_effort_timestamp_time=1.0|duration_time=0.12", 0.04).unwrap();
    close(frame_interval(last, None, 0.0).unwrap().1, 1.12);
    for duration in ["0", "-0.5", "NaN", "inf", "N/A"] {
        let timing = parse_timing(
            &format!("best_effort_timestamp_time=1.0|duration_time={duration}"),
            0.04,
        )
        .unwrap();
        close(frame_interval(timing, None, 0.0).unwrap().1, 1.04);
    }
}

#[test]
fn clipping_before_the_origin_does_not_extend_the_frames_end() {
    let (start, end) = frame_interval((0.0, 0.04), Some((0.08, 0.04)), 0.02).unwrap();
    close(start, 0.0);
    close(end, 0.06);
}

#[test]
fn malformed_order_and_excessive_timestamp_lines_are_errors() {
    assert!(frame_interval((2.0, 0.04), Some((1.9, 0.04)), 0.0).is_err());
    assert!(frame_interval((f64::MAX, f64::MAX), None, 0.0).is_err());
    let mut oversized = Cursor::new("x".repeat(16 * 1024 + 1));
    assert!(read_timing(&mut oversized, 0.04).is_err());
    assert_eq!(oversized.position(), 16 * 1024 + 1);
}

#[test]
#[ignore = "runs FFmpeg; run on the host with its FFmpeg"]
fn ffmpeg_preserves_vfr_ends_and_the_offset_between_container_and_video() {
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root =
        std::env::temp_dir().join(format!("tbd-frame-timing-{}-{unique}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let video = root.join("offset-vfr.mkv");
    let programs = Programs::default();
    let output = Run::new(&programs.ffmpeg)
        .args([
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=16x16:rate=25:duration=0.2",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=8000:duration=0.4",
            "-filter_complex",
            "[0:v]setpts=if(lt(N\\,2)\\,N\\,2*N-1)+175[v];[1:a]asetpts=PTS+6/TB[a]",
            "-map",
            "[v]",
            "-map",
            "[a]",
            "-c:v",
            "ffv1",
            "-fps_mode",
            "passthrough",
            "-c:a",
            "pcm_s16le",
            "-avoid_negative_ts",
            "disabled",
            "-y",
        ])
        .arg(&video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    close(container_origin(&programs, &video).unwrap(), 6.0);
    let probe = crate::probe::probe(&programs, &video).unwrap();
    let video_start = probe.video.unwrap().start_time_s;
    close(video_start, 7.0);
    let mut stream = FrameStream::open(&programs, &video, (16, 16), video_start, 25.0).unwrap();
    for (start, end) in [
        (1.0, 1.04),
        (1.04, 1.12),
        (1.12, 1.2),
        (1.2, 1.28),
        (1.28, 1.32),
    ] {
        let frame = stream.next_frame().unwrap().expect("source frame");
        close(frame.time_s, start);
        close(frame.end_s, end);
        assert_eq!(frame.rgb.len(), 16 * 16 * 3);
    }
    assert!(stream.next_frame().unwrap().is_none());
    stream.finish().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

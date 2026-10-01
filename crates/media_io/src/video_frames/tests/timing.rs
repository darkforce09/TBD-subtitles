use std::io::Cursor;
use std::path::PathBuf;

use super::*;

fn close(found: f64, expected: f64) {
    assert!(
        (found - expected).abs() < 0.000_001,
        "{found} vs {expected}"
    );
}

#[test]
fn presentation_timing_is_not_replaced_by_an_average_frame_rate() {
    assert_eq!(
        parse_timing(
            "pts_time=1.250|duration_time=0.080|flags=K_\n",
            PACKET_TIME,
            0.04
        ),
        Some((1.25, 0.08))
    );
    assert_eq!(
        parse_timing("pts_time=1.330|duration_time=0.040\n", PACKET_TIME, 0.04),
        Some((1.33, 0.04))
    );
    assert_eq!(
        parse_timing(
            "side_data_type=H.26[45] User Data Unregistered SEI message\n",
            PACKET_TIME,
            0.04
        ),
        None
    );
    assert_eq!(parse_timing("pts_time=NaN", PACKET_TIME, 0.04), None);
    assert_eq!(
        parse_timing("pts_time=N/A|duration_time=0.04", PACKET_TIME, 0.04),
        None
    );
}

#[test]
fn the_time_key_names_the_field_that_is_read() {
    let line = "best_effort_timestamp_time=2.5|duration_time=0.05";
    assert_eq!(parse_timing(line, PACKET_TIME, 0.04), None);
    assert_eq!(
        parse_timing(line, "best_effort_timestamp_time", 0.04),
        Some((2.5, 0.05))
    );
    assert_eq!(
        parse_timing("dts_time=1.0|pts_time=2.0", PACKET_TIME, 0.04),
        Some((2.0, 0.04))
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
    let packets = parse_packets(
        concat!(
            "pts_time=2.000|duration_time=0.040|flags=K_\n",
            "side_data_type=frame metadata\n",
            "pts_time=2.120|duration_time=0.040|flags=__\n",
            "pts_time=2.160|duration_time=N/A|flags=__\n",
        ),
        0.04,
        MAX_PACKETS,
    )
    .unwrap();
    let timeline = presentation_timeline(&packets, 2.0).unwrap();
    assert_eq!(timeline.len(), 3);
    for (&(start, end), (expected_start, expected_end)) in
        timeline
            .iter()
            .zip([(0.0, 0.12), (0.12, 0.16), (0.16, 0.2)])
    {
        close(start, expected_start);
        close(end, expected_end);
    }
}

#[test]
fn packets_in_decode_order_are_sorted_into_presentation_order() {
    let packets = parse_packets(
        concat!(
            "pts_time=0.000000|duration_time=N/A|flags=K_\n",
            "pts_time=0.166667|duration_time=N/A|flags=__\n",
            "pts_time=0.083333|duration_time=N/A|flags=__\n",
            "pts_time=0.041667|duration_time=N/A|flags=__\n",
            "pts_time=0.125000|duration_time=N/A|flags=__\n",
        ),
        0.05,
        MAX_PACKETS,
    )
    .unwrap();
    let timeline = presentation_timeline(&packets, 0.0).unwrap();
    let starts = [0.0, 0.041667, 0.083333, 0.125, 0.166667];
    assert_eq!(timeline.len(), starts.len());
    for (i, &(start, end)) in timeline.iter().enumerate() {
        close(start, starts[i]);
        close(end, starts.get(i + 1).copied().unwrap_or(0.216667));
    }
}

#[test]
fn packets_the_decoder_discards_have_no_frame() {
    let packets = parse_packets(
        concat!(
            "pts_time=-0.083333|duration_time=0.041667|flags=KD\n",
            "pts_time=0.041667|duration_time=0.041667|flags=__\n",
            "pts_time=-0.041667|duration_time=0.041667|flags=_D\n",
            "pts_time=0.000000|duration_time=0.041667|flags=K__\n",
            "pts_time=0.083333|duration_time=0.041667|flags=__C\n",
        ),
        0.04,
        MAX_PACKETS,
    )
    .unwrap();
    let starts: Vec<f64> = packets.iter().map(|&(time, _)| time).collect();
    assert_eq!(starts, [0.0, 0.041667, 0.083333]);
    assert!(discarded("pts_time=1.0|flags=_D"));
    assert!(!discarded("pts_time=1.0|flags=K_"));
    assert!(!discarded("side_data_type=DOVI configuration record"));
}

#[test]
fn the_final_frame_keeps_its_reported_duration_and_invalid_duration_falls_back() {
    let last = parse_timing("pts_time=1.0|duration_time=0.12", PACKET_TIME, 0.04).unwrap();
    close(frame_interval(last, None, 0.0).unwrap().1, 1.12);
    for duration in ["0", "-0.5", "NaN", "inf", "N/A"] {
        let timing = parse_timing(
            &format!("pts_time=1.0|duration_time={duration}"),
            PACKET_TIME,
            0.04,
        )
        .unwrap();
        close(frame_interval(timing, None, 0.0).unwrap().1, 1.04);
    }
    let bare = parse_timing("pts_time=1.0|flags=K_", PACKET_TIME, 0.04).unwrap();
    close(frame_interval(bare, None, 0.0).unwrap().1, 1.04);
}

#[test]
fn clipping_before_the_origin_does_not_extend_the_frames_end() {
    let (start, end) = frame_interval((0.0, 0.04), Some((0.08, 0.04)), 0.02).unwrap();
    close(start, 0.0);
    close(end, 0.06);
}

#[test]
fn malformed_order_repeats_and_excessive_tables_are_errors() {
    assert!(frame_interval((2.0, 0.04), Some((1.9, 0.04)), 0.0).is_err());
    assert!(frame_interval((2.0, 0.04), Some((2.0, 0.04)), 0.0).is_err());
    assert!(frame_interval((f64::MAX, f64::MAX), None, 0.0).is_err());
    let repeated = parse_packets("pts_time=1.0\npts_time=1.0\n", 0.04, MAX_PACKETS).unwrap();
    assert!(presentation_timeline(&repeated, 0.0).is_err());
    let longest = format!("pts_time=1.0|{}", "x".repeat(MAX_LINE_BYTES - 13));
    assert_eq!(parse_packets(&longest, 0.04, MAX_PACKETS).unwrap().len(), 1);
    let oversized = "x".repeat(MAX_LINE_BYTES + 1);
    let table = "pts_time=0.0\nside_data_type=x\npts_time=0.1\npts_time=0.2\n";
    assert_eq!(parse_packets(table, 0.04, 3).unwrap().len(), 3);
    for result in [
        parse_packets(&oversized, 0.04, MAX_PACKETS),
        parse_packets(table, 0.04, 2),
    ] {
        match result {
            Err(MediaError::Parse(message)) => {
                assert_eq!(message, "excessive frame timestamp table")
            }
            other => panic!("expected an excessive table, got {other:?}"),
        }
    }
}

/// A pipe that is interrupted before every byte it hands over.
struct Interrupting {
    bytes: Cursor<Vec<u8>>,
    interrupt: bool,
}

impl Read for Interrupting {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        self.interrupt = !self.interrupt;
        if self.interrupt {
            return Err(ErrorKind::Interrupted.into());
        }
        let end = buffer.len().min(1);
        self.bytes.read(&mut buffer[..end])
    }
}

#[test]
fn frames_and_timestamps_agree_one_for_one_or_fail() {
    let timeline = [(0.0, 0.04), (0.04, 0.08)];
    let mut pipe = Interrupting {
        bytes: Cursor::new(vec![1, 2, 3, 4, 5, 6]),
        interrupt: false,
    };
    let first = read_frame(&mut pipe, 3, &timeline, 0).unwrap().unwrap();
    assert_eq!((first.index, first.time_s, first.end_s), (0, 0.0, 0.04));
    assert_eq!(first.rgb, [1, 2, 3]);
    let second = read_frame(&mut pipe, 3, &timeline, 1).unwrap().unwrap();
    assert_eq!((second.index, second.time_s, second.end_s), (1, 0.04, 0.08));
    assert_eq!(second.rgb, [4, 5, 6]);
    assert!(read_frame(&mut pipe, 3, &timeline, 2).unwrap().is_none());
    assert!(!has_more(&mut pipe).unwrap());
    let early_end = read_frame(&mut Cursor::new(Vec::new()), 3, &timeline, 1);
    let partial = read_frame(&mut Cursor::new(vec![1, 2]), 3, &timeline, 0);
    let beyond = read_frame(&mut Cursor::new(vec![1, 2, 3]), 3, &timeline, 2);
    for result in [early_end, partial, beyond] {
        match result {
            Err(MediaError::Parse(message)) => {
                assert_eq!(
                    message,
                    "decoded frames and presentation timestamps disagree"
                )
            }
            Err(other) => panic!("expected a disagreement, got {other:?}"),
            Ok(_) => panic!("expected a disagreement"),
        }
    }
    assert!(has_more(&mut Cursor::new(vec![0])).unwrap());
}

#[test]
fn the_proxy_decode_skips_the_loop_filter_and_changes_nothing_else() {
    let video = Path::new("/videos/a b.mkv");
    let exact = decoder_args(video, (64, 36), Decode::Exact);
    let proxy = decoder_args(video, (64, 36), Decode::Proxy);
    assert!(!exact.iter().any(|arg| arg == "-skip_loop_filter"));
    let skip = proxy
        .iter()
        .position(|arg| arg == "-skip_loop_filter")
        .expect("proxy skips the loop filter");
    assert_eq!(proxy[skip + 1], "all");
    assert!(skip < proxy.iter().position(|arg| arg == "-i").unwrap());
    let mut without = proxy.clone();
    without.drain(skip..skip + 2);
    assert_eq!(without, exact);
    let input = exact.iter().position(|arg| arg == "-i").unwrap();
    assert_eq!(exact[input + 1], "/videos/a b.mkv");
    assert!(exact.windows(2).any(|pair| pair == ["-vf", "scale=64:36"]));
    assert!(
        exact
            .windows(2)
            .any(|pair| pair == ["-fps_mode", "passthrough"])
    );
    assert_eq!(exact.last().map(String::as_str), Some("pipe:1"));
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("frame-timing-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run FFmpeg with `args` and write `output`.
fn ffmpeg(args: &[&str], output: &Path) {
    let result = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error"])
        .args(args)
        .arg("-y")
        .arg(output)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(result.code, 0, "{}", result.stderr);
}

/// The first video stream's packet table as stored, in decode order.
fn stored_packets(video: &Path) -> Vec<String> {
    let table = Run::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_packets"])
        .args([
            "-show_entries",
            "packet=pts_time,flags",
            "-of",
            "compact=p=0",
        ])
        .arg(video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(table.code, 0, "{}", table.stderr);
    table.stdout.lines().map(str::to_string).collect()
}

/// Stream every 64x36 frame of a 24 fps `video`, checking each index and interval against the
/// timeline and the clean finish; the timeline.
fn decode_all(video: &Path, decode: Decode) -> Vec<(f64, f64)> {
    let programs = Programs::default();
    let mut stream = FrameStream::open(&programs, video, (64, 36), 0.0, 24.0, decode).unwrap();
    let timeline = stream.timeline().to_vec();
    let mut count = 0;
    while let Some(frame) = stream.next_frame().unwrap() {
        assert_eq!(frame.index, count as u64);
        assert_eq!((frame.time_s, frame.end_s), timeline[count]);
        assert_eq!(frame.rgb.len(), 64 * 36 * 3);
        count += 1;
    }
    assert_eq!(count, timeline.len());
    stream.finish().unwrap();
    timeline
}

/// Frame `i` of a constant 24 fps clip presents from `i / 24` to `(i + 1) / 24`, within the
/// container's millisecond rounding.
fn assert_constant_rate(timeline: &[(f64, f64)], frames: usize) {
    assert_eq!(timeline.len(), frames, "{timeline:?}");
    for (i, &(start, end)) in timeline.iter().enumerate() {
        assert!((start - i as f64 / 24.0).abs() < 0.002, "{i}: {start}");
        assert!((end - (i + 1) as f64 / 24.0).abs() < 0.002, "{i}: {end}");
    }
}

#[test]
fn every_lossless_frame_arrives_with_its_index_and_interval() {
    let dir = scratch("lossless");
    let video = dir.join("pattern.mkv");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x36:rate=24:duration=0.5",
            "-c:v",
            "ffv1",
            "-fps_mode",
            "passthrough",
            "-avoid_negative_ts",
            "disabled",
        ],
        &video,
    );
    assert_constant_rate(&decode_all(&video, Decode::Exact), 12);
    assert_constant_rate(&decode_all(&video, Decode::Proxy), 12);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn b_frames_stored_in_decode_order_stream_in_presentation_order() {
    let dir = scratch("bframes");
    let video = dir.join("bframes.mkv");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x36:rate=24:duration=1",
            "-c:v",
            "mpeg4",
            "-bf",
            "2",
            "-g",
            "12",
            "-fps_mode",
            "passthrough",
            "-avoid_negative_ts",
            "disabled",
        ],
        &video,
    );
    let stored: Vec<f64> = stored_packets(&video)
        .iter()
        .filter_map(|line| parse_timing(line, PACKET_TIME, 0.04))
        .map(|(time, _)| time)
        .collect();
    assert!(
        stored.windows(2).any(|pair| pair[1] < pair[0]),
        "no reordered packets: {stored:?}"
    );
    assert_constant_rate(&decode_all(&video, Decode::Exact), 24);
    assert_constant_rate(&decode_all(&video, Decode::Proxy), 24);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_mp4_cut_streams_without_the_pre_roll_its_edit_list_discards() {
    let dir = scratch("cut");
    let source = dir.join("source.mp4");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x36:rate=24:duration=1",
            "-c:v",
            "mpeg4",
            "-bf",
            "2",
            "-g",
            "12",
        ],
        &source,
    );
    let cut = dir.join("cut.mp4");
    let source_arg = source.to_string_lossy().into_owned();
    ffmpeg(&["-ss", "0.3", "-i", &source_arg, "-c", "copy"], &cut);
    let stored = stored_packets(&cut);
    let pre_roll = stored.iter().filter(|line| discarded(line)).count();
    assert!(pre_roll > 0, "the cut kept no pre-roll: {stored:?}");
    let timeline = decode_all(&cut, Decode::Exact);
    assert_eq!(timeline.len() + pre_roll, stored.len());
    assert_constant_rate(&timeline, 16);
    std::fs::remove_dir_all(dir).unwrap();
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
    let mut stream = FrameStream::open(
        &programs,
        &video,
        (16, 16),
        video_start,
        25.0,
        Decode::Exact,
    )
    .unwrap();
    assert_eq!(stream.timeline().len(), 5);
    for (index, (start, end)) in [
        (1.0, 1.04),
        (1.04, 1.12),
        (1.12, 1.2),
        (1.2, 1.28),
        (1.28, 1.32),
    ]
    .into_iter()
    .enumerate()
    {
        let frame = stream.next_frame().unwrap().expect("source frame");
        assert_eq!(frame.index, index as u64);
        close(frame.time_s, start);
        close(frame.end_s, end);
        assert_eq!(frame.rgb.len(), 16 * 16 * 3);
    }
    assert!(stream.next_frame().unwrap().is_none());
    stream.finish().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn keyframes_are_numbered_in_presentation_order_without_discarded_packets() {
    let table = concat!(
        "pts_time=-0.083333|duration_time=0.041667|flags=KD\n",
        "pts_time=0.000000|duration_time=0.041667|flags=K__\n",
        "pts_time=0.125000|duration_time=0.041667|flags=___\n",
        "pts_time=0.041667|duration_time=0.041667|flags=___\n",
        "pts_time=0.083333|duration_time=0.041667|flags=___\n",
        "pts_time=0.250000|duration_time=0.041667|flags=K__\n",
        "pts_time=0.166667|duration_time=0.041667|flags=___\n",
        "pts_time=0.208333|duration_time=0.041667|flags=___\n",
        "pts_time=N/A|duration_time=0.041667|flags=K__\n",
    );
    let keyframes = packets::parse_keyframes(table, MAX_PACKETS).unwrap();
    let indices: Vec<u64> = keyframes.iter().map(|keyframe| keyframe.index).collect();
    assert_eq!(indices, [0, 6]);
    close(keyframes[1].pts_s, 0.25);
    let timeline = presentation_timeline(&parse_packets(table, 0.04, MAX_PACKETS).unwrap(), 0.0);
    close(timeline.unwrap()[6].0, 0.25);
    assert!(packets::flagged_key("pts_time=1.0|flags=K_"));
    assert!(!packets::flagged_key("pts_time=1.0|flags=__"));
    assert!(matches!(
        packets::parse_keyframes(table, 3),
        Err(MediaError::Parse(_))
    ));
}

#[test]
fn keyframes_name_the_frames_a_closed_gop_restarts_at() {
    let dir = scratch("keyframes");
    let video = dir.join("keyframes.mkv");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=64x36:rate=24:duration=2",
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-x264-params",
            "bframes=2:keyint=12:min-keyint=12:scenecut=0",
        ],
        &video,
    );
    let programs = Programs::default();
    assert_eq!(
        packets::keyframes(&programs, &video).unwrap(),
        [0, 12, 24, 36]
    );
    let keyframes = packets::keyframe_packets(&programs, &video).unwrap();
    let timeline = timeline(&programs, &video, 0.0, 24.0).unwrap();
    for keyframe in &keyframes {
        close(keyframe.pts_s, timeline[keyframe.index as usize].0);
    }
    std::fs::remove_dir_all(dir).unwrap();
}

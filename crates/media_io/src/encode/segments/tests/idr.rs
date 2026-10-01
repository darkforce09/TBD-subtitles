use std::time::Duration;

use child_process::Run;

use super::*;
use crate::encode::segments::probe_h264_source;
use crate::video_frames::packets::keyframe_packets;

#[test]
fn printed_packets_keep_their_time_and_bytes() {
    let output = concat!(
        "[PACKET]\n",
        "pts_time=1.001000\n",
        "flags=K__\n",
        "data=\n",
        "00000000: 0000 0002 6588                           ....e.\n",
        "\n",
        "[/PACKET]\n",
        "[PACKET]\n",
        "pts_time=N/A\n",
        "data=\n",
        "00000000: 0000 0002 4188                           ....A.\n",
        "[/PACKET]\n",
        "[PACKET]\n",
        "data=\n",
        "00000000: 0000 0002 4188                           ....A.\n",
        "pts_time=2.000000\n",
        "[/PACKET]\n",
    );
    let packets = parse_packets(output);
    assert_eq!(
        packets,
        [
            PrintedPacket {
                pts_s: 1.001,
                bytes: vec![0, 0, 0, 2, 0x65, 0x88],
            },
            PrintedPacket {
                pts_s: 2.0,
                bytes: vec![0, 0, 0, 2, 0x41, 0x88],
            },
        ]
    );
}

#[test]
fn a_frame_that_is_no_keyframe_is_no_idr_and_needs_no_ffprobe() {
    let programs = Programs {
        ffprobe: "/nonexistent/ffprobe".into(),
        ..Programs::default()
    };
    let keyframes = vec![KeyframePacket {
        index: 0,
        pts_s: 0.0,
    }];
    let mut probe = IdrProbe::new(
        &programs,
        Path::new("/videos/none.mkv"),
        PacketFormat::LengthPrefixed(4),
        keyframes,
        1.0 / 24.0,
    );
    assert_eq!(probe.confirm(&[3, 7]).unwrap(), [false, false]);
    assert!(probe.confirmed().is_empty());
    assert!(probe.confirm(&[0]).is_err());
}

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("idr-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Encode three seconds of a test pattern at 24 fps with a keyframe every second into `video`,
/// with `params` for x264.
fn x264_clip(video: &Path, params: &str) {
    let output = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-f", "lavfi"])
        .args(["-i", "testsrc=size=64x36:rate=24:duration=3"])
        .args([
            "-c:v",
            "libx264",
            "-preset",
            "ultrafast",
            "-x264-params",
            params,
        ])
        .arg("-y")
        .arg(video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
}

/// Which of `video`'s keyframes open an IDR picture, read from their packets.
fn idr_keyframes(video: &Path) -> (Vec<u64>, Vec<bool>) {
    let programs = Programs::default();
    let source = probe_h264_source(&programs, video).unwrap();
    let keyframes = keyframe_packets(&programs, video).unwrap();
    let indices: Vec<u64> = keyframes.iter().map(|keyframe| keyframe.index).collect();
    let mut probe = IdrProbe::new(
        &programs,
        video,
        source.packet_format.unwrap(),
        keyframes,
        source.frame_s(),
    );
    let answers = probe.confirm(&indices).unwrap();
    assert_eq!(
        probe.confirm(&indices).unwrap(),
        answers,
        "answers are kept"
    );
    (indices, answers)
}

#[test]
fn closed_gop_keyframes_are_idr_in_matroska_and_mp4() {
    let dir = scratch("closed");
    let params = "bframes=2:keyint=24:min-keyint=24:scenecut=0";
    for name in ["closed.mkv", "closed.mp4"] {
        let video = dir.join(name);
        x264_clip(&video, params);
        let (keyframes, answers) = idr_keyframes(&video);
        assert_eq!(keyframes, [0, 24, 48], "{name}");
        assert_eq!(answers, [true, true, true], "{name}");
    }
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn open_gop_keyframes_are_not_idr() {
    let dir = scratch("open");
    let video = dir.join("open.mkv");
    x264_clip(
        &video,
        "bframes=2:open-gop=1:keyint=24:min-keyint=24:scenecut=0",
    );
    let (keyframes, answers) = idr_keyframes(&video);
    assert_eq!(keyframes, [0, 24, 48]);
    assert_eq!(answers, [true, false, false]);
    std::fs::remove_dir_all(dir).unwrap();
}

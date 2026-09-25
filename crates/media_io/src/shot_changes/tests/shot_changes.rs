use std::path::PathBuf;
use std::time::Duration;

use child_process::Run;

use super::*;

#[test]
fn reads_the_cut_times_from_the_log() {
    let log = "\
[scdet @ 0x55] lavfi.scd.score: 38.412, lavfi.scd.time: 12.5
frame=  300 fps=0.0
[scdet @ 0x55] lavfi.scd.score: 21.004, lavfi.scd.time: 3.041667
[scdet @ 0x55] lavfi.scd.score: 21.004, lavfi.scd.time: 3.041667
";
    let shots = parse(log);
    assert_eq!(shots.times_at_least(0.0), vec![3.041667, 12.5]);
    assert_eq!(shots.cuts[1].score, 38.412);
    assert_eq!(shots.times_at_least(30.0), vec![12.5]);
}

#[test]
fn a_log_without_cuts_has_none() {
    assert!(parse("Stream #0:0: Video: h264\n").cuts.is_empty());
}

#[test]
fn finds_a_hard_cut_in_a_generated_video() {
    let dir = std::env::temp_dir().join(format!("shot-changes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let video: PathBuf = dir.join("cut.mp4");
    let out = Run::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
        .arg("color=c=black:s=320x240:r=24:d=1")
        .args(["-f", "lavfi", "-i"])
        .arg("color=c=white:s=320x240:r=24:d=1")
        .args([
            "-filter_complex",
            "[0:v][1:v]concat=n=2:v=1[v]",
            "-map",
            "[v]",
        ])
        .arg(&video)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(out.code, 0, "{}", out.stderr);
    let shots = scan(&Programs::default(), &video, false, Duration::from_secs(30)).unwrap();
    assert_eq!(shots.cuts.len(), 1, "{:?}", shots.cuts);
    assert!((shots.cuts[0].time_s - 1.0).abs() < 0.1);
    assert!(
        shots.cuts[0].score > 30.0,
        "a black-to-white cut scores high"
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

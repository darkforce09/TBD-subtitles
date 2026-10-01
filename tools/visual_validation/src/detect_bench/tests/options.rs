use super::*;
use clap::Parser;

#[derive(Parser)]
struct Command {
    #[command(flatten)]
    options: Options,
}

fn parse(args: &[&str]) -> Result<Options, clap::Error> {
    Command::try_parse_from(std::iter::once("detect-bench").chain(args.iter().copied()))
        .map(|command| command.options)
}

#[test]
fn defaults_measure_two_minutes_from_ten_minutes_at_three_gibibytes() {
    let options = parse(&["/media/ep.mp4"]).expect("a video alone parses");
    assert_eq!(options.video, PathBuf::from("/media/ep.mp4"));
    assert_eq!(options.start, 600.0);
    assert_eq!(options.duration, 120.0);
    assert_eq!(options.memory_limit_mib, 3072);
    assert_eq!(options.raised_limit_mib, 4608);
    assert_eq!(options.ffmpeg_dir, None);
    assert_eq!(options.models_dir, None);
    let clip = options.clip().expect("the default clip is usable");
    assert_eq!(clip.start_s, 600.0);
    assert_eq!(clip.duration_s, 120.0);
}

#[test]
fn every_option_is_read() {
    let options = parse(&[
        "ep.mp4",
        "--start",
        "30",
        "--duration",
        "15.5",
        "--ffmpeg-dir",
        "/opt/ffmpeg",
        "--models-dir",
        "/models",
        "--runtime-dir",
        "/runtime",
        "--memory-limit-mib",
        "2048",
        "--raised-limit-mib",
        "4096",
    ])
    .expect("every option parses");
    assert_eq!(options.start, 30.0);
    assert_eq!(options.duration, 15.5);
    assert_eq!(options.ffmpeg_dir, Some(PathBuf::from("/opt/ffmpeg")));
    assert_eq!(options.models_dir, Some(PathBuf::from("/models")));
    assert_eq!(options.runtime_dir, Some(PathBuf::from("/runtime")));
    assert_eq!(options.memory_limit_mib, 2048);
    assert_eq!(options.raised_limit_mib, 4096);
}

#[test]
fn a_missing_video_or_an_unusable_clip_is_refused() {
    assert!(parse(&[]).is_err());
    assert!(parse(&["ep.mp4", "--start", "soon"]).is_err());
    let negative = parse(&["ep.mp4", "--start=-1"]).expect("a negative start parses");
    assert!(negative.clip().is_err());
    let empty = parse(&["ep.mp4", "--duration", "0"]).expect("a zero duration parses");
    assert!(empty.clip().is_err());
}

#[test]
fn a_1080p_source_gets_a_640_by_360_proxy() {
    assert_eq!(even(PROXY_WIDTH * 1080 / 1920), 360);
    assert_eq!(even(PROXY_WIDTH * 817 / 1440), 362);
}

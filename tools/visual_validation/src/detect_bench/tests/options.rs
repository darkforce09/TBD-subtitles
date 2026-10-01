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
fn every_pool_section_runs_by_default_over_three_batches_and_four_pools() {
    let options = parse(&["ep.mp4"]).expect("a video alone parses");
    assert_eq!(options.sweep_batches, vec![2, 4, 8]);
    assert_eq!(options.sweep_pools_mib, vec![1536, 2048, 2560, 3072]);
    assert_eq!(options.confirm_pools_mib, vec![1536, 2048, 2560, 3072]);
    assert_eq!(options.vram_cap_mib, DEFAULT_VRAM_CAP_MIB);
    assert_eq!(options.frames_dir, None);
    assert_eq!(options.frames_count, 6);
    let sections = options.sections().expect("the defaults are usable");
    assert_eq!(sections.grid.len(), 12);
    assert_eq!(sections.shape, None);
    assert!(sections.cuda_sweep && sections.sessions && sections.search);
    assert!(sections.tensorrt && sections.confirm);
    assert!(!sections.reference);
    assert!(
        options
            .trt_cache()
            .ends_with("tbd-subtitles-detect-bench/tensorrt")
    );
}

#[test]
fn every_pool_option_is_read() {
    let options = parse(&[
        "ep.mp4",
        "--no-decode",
        "--no-oar-ocr",
        "--no-sweep",
        "--no-sessions",
        "--no-search",
        "--no-tensorrt",
        "--no-pool-confirm",
        "--sweep-batches",
        "4,8",
        "--sweep-pools-mib",
        "2048",
        "--confirm-pools-mib",
        "2560,0,4096",
        "--shape-batch",
        "8",
        "--shape-pool-mib",
        "3072",
        "--trt-cache",
        "/work/trt",
        "--vram-cap-mib",
        "6000",
        "--frames-dir",
        "/work/frames",
        "--frames-count",
        "3",
    ])
    .expect("every option parses");
    assert!(options.no_decode && options.no_oar_ocr);
    assert_eq!(options.sweep_batches, vec![4, 8]);
    assert_eq!(options.sweep_pools_mib, vec![2048]);
    assert_eq!(options.confirm_pools_mib, vec![2560, 0, 4096]);
    assert_eq!(options.trt_cache(), PathBuf::from("/work/trt"));
    assert_eq!(options.vram_cap_mib, 6000);
    assert_eq!(options.frames_dir, Some(PathBuf::from("/work/frames")));
    assert_eq!(options.frames_count, 3);
    let sections = options.sections().expect("the options are usable");
    assert_eq!(
        sections.shape,
        Some(ScreenShape {
            batch: 8,
            pool_mib: 3072
        })
    );
    assert!(!sections.cuda_sweep && !sections.sessions && !sections.search);
    assert!(!sections.tensorrt && !sections.confirm);
    assert!(sections.reference);
    assert_eq!(sections.confirm_pools_mib, vec![2560, 4096]);
}

#[test]
fn half_a_shape_takes_the_production_default_for_the_other_half() {
    let options = parse(&["ep.mp4", "--shape-batch", "2"]).expect("parses");
    let shape = options.shape().expect("usable").expect("given");
    assert_eq!(shape.batch, 2);
    assert_eq!(shape.pool_mib, ScreenShape::INITIAL.pool_mib);
    let zero = parse(&["ep.mp4", "--shape-pool-mib", "0"]).expect("parses");
    assert!(zero.shape().is_err());
}

#[test]
fn an_empty_sweep_is_refused_only_when_a_sweep_runs() {
    let empty = parse(&["ep.mp4", "--sweep-batches", "0"]).expect("parses");
    assert!(empty.sections().is_err());
    let skipped = parse(&[
        "ep.mp4",
        "--sweep-batches",
        "0",
        "--no-sweep",
        "--no-tensorrt",
    ])
    .expect("parses");
    assert!(skipped.sections().is_ok());
    let no_frames =
        parse(&["ep.mp4", "--frames-dir", "/f", "--frames-count", "0"]).expect("parses");
    assert!(no_frames.sections().is_err());
    assert!(parse(&["ep.mp4", "--sweep-batches", "two"]).is_err());
}

#[test]
fn a_1080p_source_gets_a_640_by_360_proxy() {
    assert_eq!(even(PROXY_WIDTH * 1080 / 1920), 360);
    assert_eq!(even(PROXY_WIDTH * 817 / 1440), 362);
}

#[test]
fn confirmation_without_a_pool_is_refused_only_when_it_runs() {
    let empty = parse(&["ep.mp4", "--confirm-pools-mib", "0"]).expect("parses");
    assert!(empty.sections().is_err());
    let skipped =
        parse(&["ep.mp4", "--confirm-pools-mib", "0", "--no-pool-confirm"]).expect("parses");
    assert!(skipped.sections().is_ok());
}

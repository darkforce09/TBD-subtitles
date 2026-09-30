use super::*;
use clap::CommandFactory;
use job_model::job::{OutputFormat, Separator, WhisperModel};

use crate::settings::models::app_settings::AppSettings;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("tbd-subtitles").chain(args.iter().copied()))
}

/// The job settings the options ask for over an empty settings file.
fn job_settings(
    args: &process_command::ProcessArgs,
) -> anyhow::Result<job_model::job::JobSettings> {
    let chosen = process_command::merged(AppSettings::default(), args);
    process_command::settings(&chosen, args)
}

fn process_args(args: &[&str]) -> process_command::ProcessArgs {
    match parse(args).unwrap().command {
        Some(Command::Process(args)) => args,
        other => panic!("expected process, got {other:?}"),
    }
}

#[test]
fn the_command_definition_is_consistent() {
    Cli::command().debug_assert();
}

#[test]
fn no_subcommand_opens_the_window() {
    assert!(parse(&[]).unwrap().command.is_none());
}

#[test]
fn gui_takes_optional_videos() {
    let cli = parse(&["gui", "a.mp4", "b.mkv"]).unwrap();
    match cli.command {
        Some(Command::Gui { videos }) => assert_eq!(videos.len(), 2),
        other => panic!("expected gui, got {other:?}"),
    }
}

#[test]
fn process_needs_at_least_one_video() {
    assert!(parse(&["process"]).is_err());
    assert_eq!(process_args(&["process", "a.mp4"]).videos.len(), 1);
}

#[test]
fn process_defaults_to_the_built_in_glossary_and_the_measured_stack() {
    let settings = job_settings(&process_args(&["process", "a.mp4"])).unwrap();
    assert!(settings.glossary.iter().any(|t| t == "Doflamingo"));
    assert_eq!(settings.separator, Separator::Roformer);
    assert_eq!(settings.whisper, WhisperModel::LargeV3);
    assert_eq!(settings.cut_score, 20.0);
    assert_eq!(settings.llm_model, "sonnet");
}

#[test]
fn process_options_reach_the_settings() {
    let args = process_args(&[
        "process",
        "a.mp4",
        "--glossary",
        "none",
        "--separator",
        "mdx-net",
        "--whisper",
        "large-v3-turbo",
        "--cut-score",
        "30",
        "--audio-track",
        "1",
        "--rerun",
        "cues",
        "--rerun",
        "sound_cues",
    ]);
    let settings = job_settings(&args).unwrap();
    assert!(settings.glossary.is_empty());
    assert_eq!(settings.separator, Separator::MdxNet);
    assert_eq!(settings.whisper, WhisperModel::LargeV3Turbo);
    assert_eq!((settings.cut_score, settings.audio_track), (30.0, Some(1)));
    assert!(parse(&["process", "a.mp4", "--rerun", "asr"]).is_err());
    let missing = job_settings(&process_args(&[
        "process",
        "a.mp4",
        "--glossary",
        "/no/such.json",
    ]));
    assert!(missing.is_err());
}

#[test]
fn worker_takes_main_binary_steps_only() {
    let cli = parse(&["worker", "asr_parakeet", "/tmp/job"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Worker {
            step: StepName::AsrParakeet,
            ..
        })
    ));
    let error = parse(&["worker", "asr_whisper", "/tmp/job"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("tbd-subtitles-ggml"), "{error}");
    let error = parse(&["worker", "subtitles", "/tmp/job"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("is not a step"), "{error}");
}

#[test]
fn process_refuses_a_missing_video_by_name() {
    let error = dispatch(parse(&["process", "/no/such/video.mp4"]).unwrap()).unwrap_err();
    assert!(format!("{error:#}").contains("/no/such/video.mp4"));
}

#[test]
fn a_worker_without_a_job_fails() {
    let error = dispatch(parse(&["worker", "separation", "/no/such/job"]).unwrap()).unwrap_err();
    assert!(format!("{error:#}").contains("job.json"), "{error:#}");
}

#[test]
fn options_win_over_the_settings_file() {
    let mut file = AppSettings {
        cut_score: 25.0,
        output_format: OutputFormat::Vtt,
        ..AppSettings::default()
    };
    file.language_model.model = "opus".into();
    let kept = process_command::merged(file.clone(), &process_args(&["process", "a.mp4"]));
    assert_eq!(kept, file, "no option keeps the file's values");
    let args = process_args(&[
        "process",
        "a.mp4",
        "--cut-score",
        "30",
        "--format",
        "ass",
        "--models-dir",
        "/models",
    ]);
    let chosen = process_command::merged(file, &args);
    assert_eq!(chosen.cut_score, 30.0);
    assert_eq!(chosen.output_format, OutputFormat::Ass);
    assert_eq!(chosen.models_dir, Some(std::path::PathBuf::from("/models")));
    assert_eq!(chosen.language_model.model, "opus");
}

fn fix_args(args: &[&str]) -> fix_command::FixArgs {
    match parse(args).unwrap().command {
        Some(Command::Fix(args)) => args,
        other => panic!("expected fix, got {other:?}"),
    }
}

#[test]
fn fix_needs_a_video_and_asks_opus_unless_told_otherwise() {
    assert!(parse(&["fix"]).is_err());
    let chosen = fix_command::merged(AppSettings::default(), &fix_args(&["fix", "a.mp4"]));
    assert_eq!(chosen.language_model.fix_model, "opus");
    assert_eq!(chosen.language_model.model, "sonnet");
}

#[test]
fn fix_options_win_over_the_settings_file() {
    let args = fix_args(&[
        "fix",
        "a.mp4",
        "--model",
        "fable",
        "--processes",
        "3",
        "--work-root",
        "/big/work",
    ]);
    let chosen = fix_command::merged(AppSettings::default(), &args);
    assert_eq!(chosen.language_model.fix_model, "fable");
    assert_eq!(chosen.language_model.processes, 3);
    assert_eq!(chosen.work_root, Some(PathBuf::from("/big/work")));
    assert_eq!(args.video, PathBuf::from("a.mp4"));
}

#[test]
fn videos_without_a_subcommand_open_the_window_with_them() {
    let cli = parse(&["a.mkv", "b.mkv"]).unwrap();
    assert!(cli.command.is_none());
    let request = window_request(&cli).expect("the window opens");
    let videos = vec![PathBuf::from("a.mkv"), PathBuf::from("b.mkv")];
    assert_eq!(request, window_command::WindowRequest::open(videos));
    let message = request.hand_off();
    assert!(message.raise && !message.start, "{message:?}");
    let launch = request.launch(None);
    assert!(!launch.start && !launch.minimized);
    assert_eq!(launch.videos.len(), 2);
}

#[test]
fn gui_and_enqueue_concern_the_window_and_the_rest_do_not() {
    let gui = parse(&["gui", "a.mkv"]).unwrap();
    assert_eq!(
        window_request(&gui),
        Some(window_command::WindowRequest::open(vec![PathBuf::from(
            "a.mkv"
        )]))
    );
    let enqueue = parse(&["process", "--enqueue", "a.mkv", "b.mkv"]).unwrap();
    let request = window_request(&enqueue).expect("--enqueue goes to the window");
    assert!(request.enqueue);
    assert_eq!(request.videos.len(), 2);
    let message = request.hand_off();
    assert!(message.start && !message.raise, "{message:?}");
    let launch = request.launch(None);
    assert!(launch.start && launch.minimized);
    for args in [
        &["process", "a.mkv"][..],
        &["fix", "a.mkv"],
        &["dump", "a.mkv", "meta", "layout"],
        &["worker", "separation", "/tmp/job"],
    ] {
        assert!(window_request(&parse(args).unwrap()).is_none(), "{args:?}");
    }
}

#[test]
fn enqueue_takes_no_job_option() {
    for option in [
        &["--settings", "s.toml"][..],
        &["--work-root", "/w"],
        &["--models-dir", "/m"],
        &["--glossary", "none"],
        &["--audio-track", "1"],
        &["--separator", "mdx-net"],
        &["--whisper", "large-v3-turbo"],
        &["--cut-score", "30"],
        &["--llm-model", "opus"],
        &["--format", "vtt"],
        &["--rerun", "cues"],
    ] {
        let mut args = vec!["process", "--enqueue", "a.mkv"];
        args.extend_from_slice(option);
        let error = parse(&args).expect_err("an option with --enqueue is refused");
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::ArgumentConflict,
            "{option:?}"
        );
    }
    assert!(process_args(&["process", "--enqueue", "a.mkv"]).enqueue);
}

#[test]
fn process_run_refuses_enqueue() {
    let error = dispatch(parse(&["process", "--enqueue", "a.mkv"]).unwrap()).unwrap_err();
    assert!(format!("{error:#}").contains("--enqueue"), "{error:#}");
}

/// A scratch folder of its own for `name`, emptied.
fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("tbd-cli-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// A file at `path` with a few bytes in it, its folder made first.
fn touch(path: &std::path::Path) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, b"video").unwrap();
}

#[test]
fn a_folder_gives_every_video_under_it_without_subtitles() {
    let root = scratch("expand");
    let season = root.join("season");
    for file in [
        "b.mkv",
        "a.mp4",
        "deep/c.mkv",
        "done.mkv",
        "done.srt",
        "notes.txt",
    ] {
        touch(&season.join(file));
    }
    let single = root.join("single.mkv");
    touch(&single);
    let paths = [single.clone(), season.clone(), single.clone()];
    let videos = process_command::expand(&paths).unwrap();
    assert_eq!(
        videos,
        vec![
            single,
            season.join("a.mp4"),
            season.join("b.mkv"),
            season.join("deep").join("c.mkv"),
        ],
        "the file first, then the folder's videos sorted, each once"
    );
    // A video named is processed even with subtitles beside it.
    let done = season.join("done.mkv");
    let named = process_command::expand(std::slice::from_ref(&done)).unwrap();
    assert_eq!(named, vec![done]);
    let empty = root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let error = process_command::expand(std::slice::from_ref(&empty)).unwrap_err();
    let error = format!("{error:#}");
    assert!(error.contains(&empty.display().to_string()), "{error}");
    let missing = root.join("missing.mkv");
    let error = process_command::expand(std::slice::from_ref(&missing)).unwrap_err();
    let error = format!("{error:#}");
    assert!(error.contains(&missing.display().to_string()), "{error}");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn the_exit_code_says_whether_every_job_passed_the_quality_check() {
    let ran = |video: &str, failures: &[&str]| process_command::Ran {
        video: PathBuf::from("/videos").join(video),
        qc_failures: failures.iter().map(|f| f.to_string()).collect(),
    };
    assert_eq!(process_command::verdict(&[]), (0, None));
    let passed = [ran("a.mkv", &[]), ran("b.mkv", &[])];
    assert_eq!(process_command::verdict(&passed), (0, None));
    let (code, summary) = process_command::verdict(&[
        ran("a.mkv", &[]),
        ran("b.mkv", &["2 lines over 20 cps"]),
        ran("c.mkv", &["gaps"]),
    ]);
    assert_eq!(code, 2);
    assert_eq!(
        summary.as_deref(),
        Some("2 of 3 failed the quality check: b.mkv, c.mkv")
    );
}

#[test]
fn step_progress_prints_once_per_tenth_whatever_the_report_size() {
    use job_model::StepName;
    let printed: Vec<usize> = (0..=44_489)
        .step_by(240)
        .chain([44_489])
        .filter(|&done| process_command::enters_tenth(StepName::LocalizedVideo, done, 44_489))
        .collect();
    assert_eq!(printed.len(), 11, "{printed:?}");
    assert_eq!(printed.last(), Some(&44_489));
}

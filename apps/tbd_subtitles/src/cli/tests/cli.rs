use super::*;
use clap::CommandFactory;
use job_model::job::{Separator, WhisperModel};

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("tbd-subtitles").chain(args.iter().copied()))
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
    let settings = process_command::settings(&process_args(&["process", "a.mp4"])).unwrap();
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
    let settings = process_command::settings(&args).unwrap();
    assert!(settings.glossary.is_empty());
    assert_eq!(settings.separator, Separator::MdxNet);
    assert_eq!(settings.whisper, WhisperModel::LargeV3Turbo);
    assert_eq!((settings.cut_score, settings.audio_track), (30.0, Some(1)));
    assert!(parse(&["process", "a.mp4", "--rerun", "asr"]).is_err());
    let missing = process_command::settings(&process_args(&[
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

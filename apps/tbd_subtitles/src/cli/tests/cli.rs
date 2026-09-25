use super::*;
use clap::CommandFactory;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("tbd-subtitles").chain(args.iter().copied()))
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
    assert!(matches!(
        parse(&["process", "a.mp4"]).unwrap().command,
        Some(Command::Process { .. })
    ));
}

#[test]
fn worker_accepts_gpu_stages_only() {
    let cli = parse(&["worker", "asr", "/tmp/job"]).unwrap();
    assert!(matches!(
        cli.command,
        Some(Command::Worker {
            stage: StageName::Asr,
            ..
        })
    ));
    let error = parse(&["worker", "cues", "/tmp/job"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("runs inside the job runner"), "{error}");
    let error = parse(&["worker", "subtitles", "/tmp/job"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("is not a stage"), "{error}");
}

#[test]
fn process_refuses_a_missing_video_by_name() {
    let error = dispatch(parse(&["process", "/no/such/video.mp4"]).unwrap()).unwrap_err();
    assert!(format!("{error:#}").contains("/no/such/video.mp4"));
}

#[test]
fn process_and_worker_never_report_success_before_the_stages_exist() {
    let video = std::env::current_exe().unwrap();
    let video = video.to_str().unwrap();
    assert!(dispatch(parse(&["process", video]).unwrap()).is_err());
    assert!(dispatch(parse(&["worker", "separation", "/tmp"]).unwrap()).is_err());
}

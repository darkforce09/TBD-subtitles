use super::*;
use clap::CommandFactory;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("tbd-subtitles-ggml").chain(args.iter().copied()))
}

#[test]
fn the_command_definition_is_consistent() {
    Cli::command().debug_assert();
}

#[test]
fn only_the_whisper_steps_are_accepted() {
    for step in ["asr_whisper", "redecode_whisper"] {
        assert!(parse(&["worker", step, "/tmp/job"]).is_ok(), "{step}");
    }
    let error = parse(&["worker", "asr_parakeet", "/tmp/job"])
        .unwrap_err()
        .to_string();
    assert!(error.contains("runs in `tbd-subtitles`"), "{error}");
}

#[test]
fn a_build_without_crispasr_refuses_to_run() {
    let cli = parse(&["worker", "asr_whisper", "/no/such/job"]).unwrap();
    assert!(run(cli).is_err());
}

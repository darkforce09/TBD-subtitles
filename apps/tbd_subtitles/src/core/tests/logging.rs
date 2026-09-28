use std::ffi::OsStr;

use super::*;

#[test]
fn the_log_goes_under_xdg_state_home() {
    assert_eq!(
        log_path(Some(OsStr::new("/s")), Some(OsStr::new("/home/o"))),
        Some(PathBuf::from("/s/tbd-subtitles/tbd-subtitles.log"))
    );
}

#[test]
fn without_xdg_state_home_the_log_goes_under_local_state() {
    assert_eq!(
        log_path(Some(OsStr::new("")), Some(OsStr::new("/home/o"))),
        Some(PathBuf::from(
            "/home/o/.local/state/tbd-subtitles/tbd-subtitles.log"
        ))
    );
    assert_eq!(log_path(None, None), None);
}

#[test]
fn opening_the_log_makes_its_folder_and_empties_it() {
    let dir = std::env::temp_dir().join(format!("tbd-log-{}", std::process::id()));
    let path = dir.join("nested").join("tbd-subtitles.log");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, "old run").unwrap();
    drop(open(&path).unwrap());
    assert_eq!(fs::read_to_string(&path).unwrap(), "");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_detail_filter_parses_and_keeps_workspace_debug_lines() {
    let filter = EnvFilter::new(DETAIL);
    let text = filter.to_string();
    assert!(text.contains("pipeline=debug"), "{text}");
    assert!(text.contains("child_process=debug"), "{text}");
}

#[test]
fn the_console_is_one_buffer_for_the_whole_process() {
    assert!(Arc::ptr_eq(&console(), &console()));
}

#[test]
fn text_outputs_never_carry_a_model_call_and_the_window_always_does() {
    assert!(
        text_filter("trace")
            .to_string()
            .contains("model_exchange=off")
    );
    assert!(
        with_exchanges(EnvFilter::new("warn"))
            .to_string()
            .contains("model_exchange=trace")
    );
    let only = exchanges_only().to_string();
    assert!(
        only.contains("off") && only.contains("model_exchange=trace"),
        "{only}"
    );
}

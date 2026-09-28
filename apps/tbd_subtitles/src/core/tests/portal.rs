use super::*;

#[test]
fn a_chosen_video_uri_becomes_its_path() {
    assert_eq!(
        file_path(
            "file:///run/media/system/Main_storage/Media/one_pace/%5BMuhn%20Pace%5D%20Dressrosa%2012.mp4"
        ),
        Some(PathBuf::from(
            "/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 12.mp4"
        ))
    );
}

#[test]
fn the_portal_answer_decides_whether_an_open_failed() {
    assert_eq!(answered(Ok(())), Ok(()));
    assert_eq!(
        answered(Err(ashpd::Error::Response(ResponseError::Cancelled))),
        Ok(()),
        "closing the desktop's \"open with\" chooser is no failure"
    );
    let refused = answered(Err(ashpd::Error::Response(ResponseError::Other)));
    assert!(refused.is_err(), "a refusal is a failure: {refused:?}");
}

#[test]
fn a_portal_uri_with_accents_and_localhost_decodes() {
    assert_eq!(
        file_path("file://localhost/v/Se%C3%B1or%20Pink.mkv"),
        Some(PathBuf::from("/v/Señor Pink.mkv"))
    );
}

#[test]
fn only_local_files_are_paths() {
    assert_eq!(file_path("https://example.com/a.mp4"), None);
    assert_eq!(file_path("file://server/share/a.mp4"), None);
}

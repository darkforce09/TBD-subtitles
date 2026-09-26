use super::*;

#[test]
fn a_video_path_becomes_a_file_uri_and_back() {
    let path =
        Path::new("/run/media/system/Main_storage/Media/one_pace/[Muhn Pace] Dressrosa 12.mp4");
    let uri = file_uri(path);
    assert_eq!(
        uri,
        "file:///run/media/system/Main_storage/Media/one_pace/%5BMuhn%20Pace%5D%20Dressrosa%2012.mp4"
    );
    assert_eq!(file_path(&uri), Some(path.to_path_buf()));
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

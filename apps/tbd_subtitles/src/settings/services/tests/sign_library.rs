use super::*;
use job_model::onscreen::{LetteringStyle, LibrarySign};

#[test]
fn the_library_is_measured_and_cleared_on_a_thread() {
    let root = std::env::temp_dir().join(format!("tbd-sign-library-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let path = root.join(pipeline::library::FILE_NAME);
    assert_eq!(size(path.clone()), Ok((0, 0)), "no file yet");
    Library::at(path.clone())
        .record(LibrarySign {
            japanese: "王宮".into(),
            crop_hash: 1,
            english: "Royal Palace".into(),
            confidence: 1.0,
            style: LetteringStyle {
                fill_rgb: [0, 0, 0],
                outline_rgb: None,
                outline_px: 0.0,
                soft_outline: false,
                stroke_px: 2.0,
                line_height_px: 20.0,
            },
            patch_png: vec![0; 64],
            mask_png: vec![0; 16],
            episodes: vec!["d11".into()],
            added_s: 0,
        })
        .expect("recorded");
    let wake: Wake = std::sync::Arc::new(|| {});
    let (signs, bytes) = start(path.clone(), size, wake.clone())
        .recv()
        .expect("an answer")
        .expect("a size");
    assert_eq!(signs, 1);
    assert!(bytes > 0);
    let cleared = start(path, clear, wake).recv().expect("an answer");
    assert_eq!(cleared.map(|(signs, _)| signs), Ok(0));
    let _ = std::fs::remove_dir_all(&root);
}

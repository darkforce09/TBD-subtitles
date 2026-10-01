use super::*;
use crate::onscreen_text::detect::fixtures::Temporary;

#[test]
fn crops_are_written_whole_and_keyframes_at_most_1280_pixels_wide() {
    let root = Temporary::new("writer");
    std::fs::create_dir_all(root.0.join("visual/keyframes")).unwrap();
    let mut writer = PngThread::start(&root.0);
    writer
        .send(PngJob::Keyframe {
            path: PathBuf::from("visual/keyframes/wide.png"),
            still: RgbImage::new(1920, 1080),
        })
        .unwrap();
    writer
        .send(PngJob::Keyframe {
            path: PathBuf::from("visual/keyframes/small.png"),
            still: RgbImage::new(640, 360),
        })
        .unwrap();
    writer
        .send(PngJob::Crop {
            path: PathBuf::from("crop.png"),
            image: RgbImage::new(140, 40),
        })
        .unwrap();
    writer.finish().unwrap();
    let size = |name: &str| image::image_dimensions(root.0.join(name)).unwrap();
    assert_eq!(size("visual/keyframes/wide.png"), (1280, 720));
    assert_eq!(size("visual/keyframes/small.png"), (640, 360));
    assert_eq!(size("crop.png"), (140, 40));
}

#[test]
fn a_failed_write_comes_back_from_send_or_finish() {
    let root = Temporary::new("writer-error");
    let mut writer = PngThread::start(&root.0);
    let mut failed = false;
    for count in 0..64 {
        let sent = writer.send(PngJob::Crop {
            path: PathBuf::from(format!("missing/folder/crop-{count}.png")),
            image: RgbImage::new(4, 4),
        });
        if let Err(error) = sent {
            assert!(error.to_string().contains("missing/folder"), "{error}");
            failed = true;
            break;
        }
    }
    if !failed {
        let error = writer.finish().unwrap_err();
        assert!(error.to_string().contains("missing/folder"), "{error}");
    }
}

#[test]
fn finishing_a_writer_with_nothing_to_write_succeeds() {
    let root = Temporary::new("writer-empty");
    PngThread::start(&root.0).finish().unwrap();
}

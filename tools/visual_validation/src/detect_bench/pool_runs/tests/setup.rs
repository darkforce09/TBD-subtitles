use super::*;
use inference::ocr::SearchMode;
use inference::ocr::pool::ScreenShape;
use job_model::onscreen::DetectorEngine;
use media_io::yuv::{Matrix, Range};

/// A yuv420p picture of one luma value with neutral chroma.
fn flat(width: usize, height: usize, luma: u8) -> Vec<u8> {
    let mut bytes = vec![luma; width * height];
    bytes.resize(width * height * 3 / 2, 128);
    bytes
}

#[test]
fn a_picture_converts_into_a_frame_padded_with_black_rows() {
    let colour = Coefficients::new(Matrix::Bt709, Range::Limited);
    let frame = padded_frame(&flat(8, 34, 235), (8, 34), &colour).expect("a usable picture");
    assert_eq!(
        (frame.width, frame.height, frame.padded_height),
        (8, 34, 64)
    );
    assert_eq!(frame.rgb.len(), 8 * 64 * 3);
    let picture = 8 * 34 * 3;
    assert!(frame.rgb[..picture].iter().all(|&value| value == 255));
    assert!(frame.rgb[picture..].iter().all(|&value| value == 0));
}

#[test]
fn a_buffer_of_the_wrong_size_is_refused() {
    let colour = Coefficients::new(Matrix::Bt709, Range::Limited);
    assert!(padded_frame(&flat(8, 32, 16)[1..], (8, 32), &colour).is_err());
    assert!(padded_frame(&flat(8, 32, 16), (8, 30), &colour).is_err());
}

#[test]
fn frames_are_cut_into_numbered_jobs_with_a_short_last_one() {
    let frame = PaddedFrame {
        width: 2,
        height: 2,
        padded_height: 32,
        rgb: vec![0; 2 * 32 * 3],
    };
    let frames = vec![frame; 10];
    let jobs = jobs(&frames, 4);
    let sizes: Vec<(u64, usize)> = jobs.iter().map(|j| (j.seq, j.frames.len())).collect();
    assert_eq!(sizes, vec![(0, 4), (1, 4), (2, 2)]);
    assert!(jobs.iter().all(|job| job.priority == Priority::Screen));
    assert_eq!(super::jobs(&frames, 0).len(), 10);
}

#[test]
fn a_run_becomes_the_pool_options_it_names() {
    let setup = Setup {
        models_root: PathBuf::from("/models"),
        identity: EngineIdentity {
            gpu_name: "NVIDIA GeForce RTX 3070".into(),
            driver: "580.95.05".into(),
            tensorrt_version: "10.14.1.48".into(),
        },
        cache_dir: PathBuf::from("/tmp/bench/tensorrt"),
        vram_cap_mib: 6656,
        frame: (1920, 1080),
        baseline_mib: 900,
    };
    let shape = ScreenShape {
        batch: 8,
        pool_mib: 3072,
    };
    let run = PoolRun {
        search: SearchMode::Deterministic,
        ..PoolRun::tensorrt(shape, 1, false)
    };
    let options = setup.options(&run);
    assert_eq!(options.engine, DetectorEngine::TensorRt);
    assert_eq!(options.search, SearchMode::Deterministic);
    assert_eq!(options.shape, shape);
    assert_eq!(options.sessions, 1);
    assert!(!options.tensorrt_fp16);
    assert_eq!(options.cache_dir, PathBuf::from("/tmp/bench/tensorrt"));
    assert_eq!((options.frame_width, options.frame_height), (1920, 1080));
    assert_eq!(options.vram_cap_mib, 6656);
    assert_eq!(options.identity, setup.identity);
}

#[test]
fn the_identity_takes_the_card_and_driver_and_the_bundled_tensorrt() {
    let device = DeviceInfo {
        name: "NVIDIA GeForce RTX 3070".into(),
        driver: "580.95.05".into(),
        total_mib: 8192,
        free_mib: 5600,
    };
    let identity = identity(Some(&device));
    assert_eq!(identity.gpu_name, "NVIDIA GeForce RTX 3070");
    assert_eq!(identity.driver, "580.95.05");
    assert_eq!(
        identity.tensorrt_version,
        inference::model_store::manifest::TENSORRT_VERSION
    );
    let unknown = super::identity(None);
    assert!(unknown.gpu_name.is_empty() && unknown.driver.is_empty());
}

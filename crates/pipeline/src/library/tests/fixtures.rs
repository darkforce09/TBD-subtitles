//! What the library's tests share: sign pictures, scratch folders and signs.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use image::{GrayImage, Luma};
use job_model::onscreen::{
    LetteringStyle, LibrarySign, TextOccurrence, TextPresentation, TextProvenance,
};

/// A 128 by 64 grey picture of blocks whose brightness follows `seed`, like strokes on a sign.
pub(crate) fn sign(seed: u64) -> GrayImage {
    let mut state = seed;
    let mut blocks = [[0u8; 16]; 8];
    for row in &mut blocks {
        for cell in row.iter_mut() {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *cell = (state >> 56) as u8;
        }
    }
    GrayImage::from_fn(128, 64, |x, y| {
        Luma([blocks[(y / 8) as usize][(x / 8) as usize]])
    })
}

/// A fresh folder under the temporary folder, removed when dropped.
pub(crate) struct Scratch(pub(crate) PathBuf);

impl Scratch {
    pub(crate) fn new(name: &str) -> Scratch {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-library-{name}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Scratch(path)
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn style() -> LetteringStyle {
    LetteringStyle {
        fill_rgb: [250, 250, 240],
        outline_rgb: Some([20, 10, 10]),
        outline_px: 2.0,
        soft_outline: false,
        stroke_px: 3.5,
        line_height_px: 36.0,
    }
}

/// An approved sign of `japanese` reading `english`, first recorded by `job`.
pub(crate) fn library_sign(
    japanese: &str,
    crop_hash: u64,
    english: &str,
    job: &str,
) -> LibrarySign {
    LibrarySign {
        japanese: japanese.into(),
        crop_hash,
        english: english.into(),
        confidence: 0.95,
        style: style(),
        patch_png: vec![1, 2, 3],
        mask_png: vec![4, 5],
        episodes: vec![job.into()],
        added_s: 1_790_000_000,
    }
}

/// The crop hash of the sign picture `seed`.
pub(crate) fn hash(seed: u64) -> u64 {
    crate::library::key::difference_hash(&image::DynamicImage::ImageLuma8(sign(seed)))
}

/// An occurrence reading `japanese` whose crop, written under `root`, is the sign picture `seed`.
pub(crate) fn occurrence(root: &Path, id: &str, japanese: &str, seed: u64) -> TextOccurrence {
    let crop = PathBuf::from(format!("visual/crops/{id}.png"));
    std::fs::create_dir_all(root.join("visual/crops")).unwrap();
    sign(seed).save(root.join(&crop)).unwrap();
    TextOccurrence {
        id: id.into(),
        start_s: 1.0,
        end_s: 3.0,
        japanese: japanese.into(),
        english: Some(format!("English of {id}")),
        confidence: 0.9,
        crops: vec![crop],
        frames: Vec::new(),
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: false,
        rendered: None,
        source_fingerprint: None,
        keyframe: None,
        ruby: Vec::new(),
    }
}

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use image::{Rgb, Rgba};
use job_model::onscreen::Plate;

use super::fixtures::{occurrence, plate, quad, rect, replaced};
use super::verdict::{JAPANESE_LEFT, UNREADABLE};
use super::*;
use crate::localize::colour::{Matrix, Range};
use crate::localize::motion::Motion;

struct Temporary(PathBuf);

impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-verify-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(path.join("visual/patches")).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A black video of 20 frames, 64 × 48, that records every region decoded.
struct Black {
    timeline: Vec<(f64, f64)>,
    decoded: Vec<(PixelRect, u64)>,
}

impl RegionSource for Black {
    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn frame_size(&self) -> (u32, u32) {
        (64, 48)
    }

    fn frames(
        &mut self,
        rect: PixelRect,
        first: u64,
        last: u64,
        visit: &mut dyn FnMut(u64, RgbImage) -> TextResult<()>,
    ) -> TextResult<()> {
        for index in first..=last {
            self.decoded.push((rect, index));
            visit(
                index,
                RgbImage::from_pixel(rect.width, rect.height, Rgb([0; 3])),
            )?;
        }
        Ok(())
    }
}

/// Finds one line over the middle of the picture and reads it as the English when its centre is light,
/// else as Japanese; `double` reads the English twice.
struct Scripted {
    english: String,
    double: bool,
}

impl ReadBack for Scripted {
    fn find(&mut self, image: &RgbImage) -> TextResult<Vec<(Quad, f64)>> {
        let (w, h) = (f64::from(image.width()), f64::from(image.height()));
        Ok(vec![(quad(w / 4.0, h / 4.0, w * 0.75, h * 0.75), 0.9)])
    }

    fn read(&mut self, image: &RgbImage) -> TextResult<(String, f64)> {
        let centre = image.get_pixel(image.width() / 2, image.height() / 2).0;
        if centre[0] > 128 {
            let text = if self.double {
                format!("{0} {0}", self.english)
            } else {
                self.english.clone()
            };
            Ok((text, 0.95))
        } else {
            Ok(("幹部塔".into(), 0.9))
        }
    }
}

/// A plate whose patch, saved under `root`, is `colour` over the whole rectangle.
fn patched(root: &Path, name: &str, span: (u64, u64), at: PixelRect, colour: Rgba<u8>) -> Plate {
    let path = PathBuf::from(format!("visual/patches/{name}.png"));
    RgbaImage::from_pixel(at.width, at.height, colour)
        .save(root.join(&path))
        .unwrap();
    Plate {
        patch: Some(path),
        shifted: Vec::new(),
        ..plate(span.0, span.1, at, [0.0, 0.0])
    }
}

struct Fixture {
    _dir: Temporary,
    composed: ReplacementDocument,
    text: TextDocument,
    root: PathBuf,
}

/// "tower" lettered white over frames 0–9, "yard" whose patch is clear over frames 10–19, and a
/// fallback that is never checked.
fn fixture() -> Fixture {
    let dir = Temporary::new();
    let root = dir.0.clone();
    let white = Rgba([255, 255, 255, 255]);
    let clear = Rgba([255, 255, 255, 0]);
    let tower = patched(&root, "tower", (0, 9), rect(8, 8, 24, 16), white);
    let yard = patched(&root, "yard", (10, 19), rect(8, 24, 24, 16), clear);
    let mut left = replaced(
        "left",
        vec![plate(0, 19, rect(40, 8, 16, 16), [0.0; 2])],
        10.0,
    );
    left.status = ReplaceStatus::Fallback("The writing has no background plates".into());
    let composed = ReplacementDocument {
        width: 64,
        height: 48,
        frame_count: 20,
        texts: vec![
            replaced("tower", vec![tower], 10.0),
            replaced("yard", vec![yard], 10.0),
            left,
        ],
    };
    let text = TextDocument {
        width: 64,
        height: 48,
        decoded_frames: 20,
        occurrences: vec![
            occurrence("tower", "Tower", quad(10.0, 10.0, 30.0, 20.0), Vec::new()),
            occurrence("yard", "Yard", quad(10.0, 26.0, 30.0, 36.0), Vec::new()),
            occurrence("left", "Left", quad(42.0, 10.0, 54.0, 20.0), Vec::new()),
        ],
        ..TextDocument::default()
    };
    Fixture {
        _dir: dir,
        composed,
        text,
        root,
    }
}

fn run(
    fixture: &Fixture,
    reader: &mut Scripted,
    only: Option<&[String]>,
) -> (Verified, Black, usize, Vec<(usize, usize)>) {
    let request = Request {
        composed: &fixture.composed,
        text: &fixture.text,
        root: &fixture.root,
        conversion: Conversion {
            matrix: Matrix::Bt709,
            range: Range::Limited,
            bits: 8,
        },
        only,
        motion: &Motion::default(),
    };
    let mut source = Black {
        timeline: (0..20).map(|i| (f64::from(i), f64::from(i + 1))).collect(),
        decoded: Vec::new(),
    };
    let mut observed = 0;
    let heard = Mutex::new(Vec::new());
    let verified = verify(
        &request,
        &mut source,
        reader,
        &mut |sample: &Sample, picture: &RgbImage| {
            assert!(picture.width() > sample.area.region.width);
            observed += 1;
        },
        &|done, total| heard.lock().unwrap().push((done, total)),
    )
    .unwrap();
    (verified, source, observed, heard.into_inner().unwrap())
}

/// The readings of occurrence `id`, in frame order.
fn readings<'v>(verified: &'v Verified, id: &str) -> Vec<&'v VerifyReading> {
    verified
        .readings
        .iter()
        .filter(|(of, _)| of == id)
        .map(|(_, reading)| reading)
        .collect()
}

#[test]
fn lettering_that_reads_back_stays_and_japanese_left_falls_back() {
    let fixture = fixture();
    let mut reader = Scripted {
        english: "Tower".into(),
        double: false,
    };
    let (verified, source, observed, heard) = run(&fixture, &mut reader, None);
    let texts = &verified.replacements.document.texts;
    assert_eq!(texts[0].status, ReplaceStatus::Baked);
    assert_eq!(
        texts[1].status,
        ReplaceStatus::Fallback(JAPANESE_LEFT.into())
    );
    assert_eq!(texts[2].status, fixture.composed.texts[2].status);
    let frames =
        |id: &str| -> Vec<u64> { readings(&verified, id).iter().map(|r| r.frame).collect() };
    assert_eq!(frames("tower"), vec![0, 4, 9]);
    assert_eq!(frames("yard"), vec![10, 14, 19]);
    assert!(verified.replacements.check("left").is_none());
    assert!(
        readings(&verified, "tower")
            .iter()
            .all(|r| r.passed && r.similarity == 1.0)
    );
    assert_eq!(
        verified.replacements.check("tower"),
        Some(&TextCheck {
            id: "tower".into(),
            samples: 3,
            passed: true,
        })
    );
    assert!(!verified.replacements.check("yard").unwrap().passed);
    assert!(
        readings(&verified, "yard")
            .iter()
            .all(|r| r.japanese_found == "幹部塔")
    );
    assert_eq!(observed, 6);
    assert_eq!(heard.first(), Some(&(0, 6)));
    assert_eq!(heard.last(), Some(&(6, 6)));
    assert!(
        source
            .decoded
            .iter()
            .all(|(region, _)| { region.x % 2 == 0 && region.y % 2 == 0 && region.width % 2 == 0 })
    );
    assert_eq!(
        verified.replacements.document.texts[0].plates,
        fixture.composed.texts[0].plates
    );
}

#[test]
fn doubled_lettering_falls_back_as_unreadable() {
    let fixture = fixture();
    let mut reader = Scripted {
        english: "Tower".into(),
        double: true,
    };
    let only = ["tower".to_string()];
    let (verified, source, observed, _) = run(&fixture, &mut reader, Some(&only));
    assert_eq!(
        verified.replacements.document.texts[0].status,
        ReplaceStatus::Fallback(UNREADABLE.into())
    );
    assert_eq!(
        verified.replacements.document.texts[1].status,
        ReplaceStatus::Baked
    );
    assert_eq!(verified.replacements.checks.len(), 1);
    assert_eq!(observed, 3);
    assert!(source.decoded.iter().all(|(_, frame)| *frame < 10));
}

#[test]
fn a_patch_blends_only_while_its_plate_lasts() {
    let mut fixture = fixture();
    // The tower's patch ends at frame 4 while the occurrence runs on, uncovered, to frame 9.
    let text = &mut fixture.composed.texts[0];
    let mut late = text.plates[0].clone();
    text.plates[0].last_frame = 4;
    late.first_frame = 5;
    late.patch = None;
    late.rect = rect(8, 8, 24, 16);
    text.plates.push(late);
    let mut reader = Scripted {
        english: "Tower".into(),
        double: false,
    };
    let only = ["tower".to_string()];
    let (verified, _, _, _) = run(&fixture, &mut reader, Some(&only));
    let passed: Vec<(u64, bool)> = readings(&verified, "tower")
        .iter()
        .map(|r| (r.frame, r.passed))
        .collect();
    assert_eq!(passed, vec![(0, true), (4, true), (5, false), (9, false)]);
    assert_eq!(
        verified.replacements.document.texts[0].status,
        ReplaceStatus::Fallback(JAPANESE_LEFT.into())
    );
}

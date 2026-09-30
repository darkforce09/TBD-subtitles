use std::path::PathBuf;

use image::{GrayImage, Luma, Rgb, RgbImage};
use job_model::onscreen::{
    PixelRect, Plate, Point, TextFrame, TextKeyframe, TextPresentation, TextProvenance,
};

use super::*;

const GREY: [u8; 3] = [90, 90, 90];

fn fonts() -> PathBuf {
    inference::model_store::models_dir()
        .expect("model folder")
        .join("latin-fonts")
}

fn job(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("compose_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("visual/plates")).unwrap();
    root
}

fn rect_quad(x: f64, y: f64, width: f64, height: f64) -> Quad {
    Quad([
        Point { x, y },
        Point { x: x + width, y },
        Point {
            x: x + width,
            y: y + height,
        },
        Point { x, y: y + height },
    ])
}

fn occurrence(id: &str, english: Option<&str>, quad: Quad) -> TextOccurrence {
    TextOccurrence {
        id: id.into(),
        start_s: 0.4,
        end_s: 2.4,
        japanese: "\u{5E97}".into(),
        english: english.map(str::to_string),
        confidence: 1.0,
        crops: Vec::new(),
        frames: vec![TextFrame {
            time_s: 0.4,
            end_s: 2.4,
            quad,
            confidence: 1.0,
            surface_rgb: None,
        }],
        provenance: TextProvenance::default(),
        presentation: TextPresentation::default(),
        warnings: Vec::new(),
        reviewed: true,
        rendered: None,
        source_fingerprint: None,
        keyframe: Some(TextKeyframe {
            time_s: 1.4,
            image: PathBuf::from("visual/keyframes/k.png"),
        }),
    }
}

fn style(line_height: f64) -> LetteringStyle {
    LetteringStyle {
        fill_rgb: [255, 255, 255],
        outline_rgb: None,
        outline_px: 0.0,
        soft_outline: false,
        stroke_px: 0.1 * line_height,
        line_height_px: line_height,
    }
}

/// A text with two plates over `rect`: frames 10..=29 in place and 30..=59 shifted 5 px right.
/// The plates are flat grey with a masked band where the strokes were.
fn replaced(root: &Path, id: &str, rect: PixelRect, line_height: f64) -> ReplacedText {
    let mut plates = Vec::new();
    for (index, (first, last, shift)) in [(10, 29, 0.0), (30, 59, 5.0)].into_iter().enumerate() {
        let stem = id.replace('/', "_");
        let name = |kind: &str| PathBuf::from(format!("visual/plates/{stem}_{index}_{kind}.png"));
        RgbImage::from_pixel(rect.width, rect.height, Rgb(GREY))
            .save(root.join(name("plate")))
            .unwrap();
        RgbImage::from_fn(rect.width, rect.height, |x, _| Rgb([(x % 256) as u8, 0, 0]))
            .save(root.join(name("source")))
            .unwrap();
        GrayImage::from_fn(rect.width, rect.height, |x, y| {
            let inside = x * 10 >= rect.width * 4
                && x * 10 < rect.width * 6
                && y * 10 >= rect.height * 4
                && y * 10 < rect.height * 6;
            Luma([if inside { 255 } else { 0 }])
        })
        .save(root.join(name("mask")))
        .unwrap();
        plates.push(Plate {
            first_frame: first,
            last_frame: last,
            rect,
            shift: [shift, 0.0],
            scale: 1.0,
            source: name("source"),
            mask: name("mask"),
            plate: Some(name("plate")),
            patch: None,
        });
    }
    ReplacedText {
        id: id.into(),
        first_frame: 10,
        last_frame: 59,
        status: ReplaceStatus::Pending,
        style: Some(style(line_height)),
        container: None,
        plates,
        preview: None,
    }
}

fn documents(
    texts: Vec<ReplacedText>,
    occurrences: Vec<TextOccurrence>,
) -> (ReplacementDocument, TextDocument) {
    let replacement = ReplacementDocument {
        width: 1920,
        height: 1080,
        frame_count: 100,
        texts,
    };
    let text = TextDocument {
        width: 1920,
        height: 1080,
        occurrences,
        ..TextDocument::default()
    };
    (replacement, text)
}

const RECT: PixelRect = PixelRect {
    x: 100,
    y: 100,
    width: 400,
    height: 160,
};

#[test]
fn a_missing_font_is_an_error() {
    let root = job("no_font");
    let (mut document, text) = documents(Vec::new(), Vec::new());
    assert!(compose(&mut document, &text, &root, &root, &|_, _| {}).is_err());
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
#[ignore = "needs the latin-fonts model"]
fn a_synthetic_plate_is_lettered_and_baked() {
    let root = job("baked");
    let quad = rect_quad(140.0, 130.0, 320.0, 100.0);
    let (mut document, text) = documents(
        vec![replaced(&root, "sign/1", RECT, 100.0)],
        vec![occurrence("sign/1", Some("SHOP"), quad)],
    );
    let calls = std::sync::Mutex::new(Vec::new());
    compose(&mut document, &text, &root, &fonts(), &|done, total| {
        calls.lock().unwrap().push((done, total));
    })
    .unwrap();
    assert_eq!(*calls.lock().unwrap(), vec![(0, 1), (1, 1)]);
    let item = &document.texts[0];
    assert_eq!(item.status, ReplaceStatus::Baked);
    assert_eq!(item.container, None);
    document.validate().unwrap();
    assert_eq!(
        item.preview.as_deref(),
        Some(Path::new("visual/patches/sign_1/preview.png"))
    );
    assert!(root.join("visual/patches/sign_1/preview.png").exists());
    for (index, plate) in item.plates.iter().enumerate() {
        let path = plate.patch.clone().unwrap();
        assert_eq!(
            path,
            PathBuf::from(format!("visual/patches/sign_1/{index}.png"))
        );
        let patch = image::open(root.join(&path)).unwrap().into_rgba8();
        assert_eq!(patch.dimensions(), (RECT.width, RECT.height));
        // Lettering inside the writing's quad (plate pixels 40..360 × 30..130, shifted).
        let shift = plate.shift[0] as u32;
        let letters = (40 + shift..360 + shift)
            .flat_map(|x| (30..130).map(move |y| (x, y)))
            .filter(|&(x, y)| patch.get_pixel(x, y).0 == [255, 255, 255, 255])
            .count();
        assert!(
            letters > 500,
            "plate {index}: {letters} opaque lettering pixels"
        );
        // Far from the mask and the writing: transparent.
        assert_eq!(patch.get_pixel(2, 2).0[3], 0);
        assert_eq!(patch.get_pixel(RECT.width - 3, RECT.height - 3).0[3], 0);
        // The masked band is covered.
        assert!(patch.get_pixel(RECT.width / 2, RECT.height / 2).0[3] > 0);
        for pixel in patch.pixels() {
            if pixel.0[3] == 0 {
                assert_eq!(&pixel.0[..3], &GREY);
            }
        }
    }
    let first = std::fs::read(root.join("visual/patches/sign_1/0.png")).unwrap();
    compose_again_is_identical(&root, &text, &first);
    std::fs::remove_dir_all(&root).unwrap();
}

fn compose_again_is_identical(root: &Path, text: &TextDocument, first: &[u8]) {
    let quad = text.occurrences[0].frames[0].quad;
    let (mut again, _) = documents(
        vec![replaced(root, "sign/1", RECT, 100.0)],
        vec![occurrence("sign/1", Some("SHOP"), quad)],
    );
    compose(&mut again, text, root, &fonts(), &|_, _| {}).unwrap();
    assert_eq!(
        std::fs::read(root.join("visual/patches/sign_1/0.png")).unwrap(),
        first
    );
}

#[test]
#[ignore = "needs the latin-fonts model"]
fn unreadable_or_undrawable_writing_falls_back() {
    let root = job("fallback");
    let tiny = PixelRect {
        x: 1000,
        y: 600,
        width: 60,
        height: 30,
    };
    let (mut document, text) = documents(
        vec![
            replaced(&root, "small", tiny, 12.0),
            replaced(&root, "han", RECT, 100.0),
            replaced(&root, "blank", RECT, 100.0),
        ],
        vec![
            occurrence(
                "small",
                Some("An announcement far too long for this"),
                rect_quad(1010.0, 605.0, 40.0, 14.0),
            ),
            occurrence(
                "han",
                Some("Café \u{4E2D}"),
                rect_quad(140.0, 130.0, 320.0, 100.0),
            ),
            occurrence("blank", None, rect_quad(140.0, 130.0, 320.0, 100.0)),
        ],
    );
    compose(&mut document, &text, &root, &fonts(), &|_, _| {}).unwrap();
    assert_eq!(
        document.texts[0].status,
        ReplaceStatus::Fallback(TOO_SMALL.into())
    );
    assert_eq!(
        document.texts[1].status,
        ReplaceStatus::Fallback("The font has no glyph for “\u{4E2D}”".into())
    );
    assert_eq!(
        document.texts[2].status,
        ReplaceStatus::Fallback("The writing has no English translation".into())
    );
    for item in &document.texts {
        assert!(item.plates.iter().all(|plate| plate.patch.is_none()));
        assert!(item.preview.is_none());
    }
    assert!(!root.join("visual/patches/small").exists());
    document.validate().unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
#[ignore = "needs the latin-fonts model"]
fn neighbouring_writing_shares_a_container_and_its_size_ratio() {
    let root = job("container");
    // A title (line height 100) in a roomy box above a caption (line height 50) whose long
    // English must shrink; both on screen together, 20 px apart.
    let title = rect_quad(140.0, 130.0, 320.0, 100.0);
    let caption = rect_quad(140.0, 250.0, 160.0, 50.0);
    let lower = PixelRect {
        x: 100,
        y: 240,
        width: 400,
        height: 80,
    };
    let (mut document, text) = documents(
        vec![
            replaced(&root, "title", RECT, 100.0),
            replaced(&root, "caption", lower, 50.0),
        ],
        vec![
            occurrence("title", Some("SHOP"), title),
            occurrence("caption", Some("Open every day until late"), caption),
        ],
    );
    let font = LetteringFont::open(&fonts()).unwrap();
    let prepared: Vec<Prepared> = (0..2)
        .map(|index| Prepared {
            index,
            ..prepare(
                &document.texts[index],
                Some(&text.occurrences[index]),
                &font,
            )
            .unwrap()
        })
        .collect();
    let members: Vec<Member> = prepared.iter().map(|ready| ready.member).collect();
    assert_eq!(containers::group(&members), vec![vec![0, 1]]);
    let fitted = fit_group(&font, &prepared, &[0, 1], 14.0).unwrap();
    let caps: Vec<f64> = fitted
        .iter()
        .map(|(_, layout)| layout.as_ref().unwrap().cap)
        .collect();
    assert!(caps[1] < 0.7 * 50.0, "the caption shrinks: {caps:?}");
    assert!(
        (caps[0] / caps[1] - 2.0).abs() < 1e-9,
        "ratio kept: {caps:?}"
    );

    compose(&mut document, &text, &root, &fonts(), &|_, _| {}).unwrap();
    for item in &document.texts {
        assert_eq!(item.status, ReplaceStatus::Baked);
        assert_eq!(item.container.as_deref(), Some("title"));
    }
    document.validate().unwrap();
    std::fs::remove_dir_all(&root).unwrap();
}

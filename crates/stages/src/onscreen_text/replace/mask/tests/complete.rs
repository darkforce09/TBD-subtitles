use image::{GrayImage, Luma, Rgb, RgbImage};
use job_model::onscreen::{LetteringStyle, PixelRect};

use super::super::cluster::{Clusters, lab};
use super::super::fixtures::rect_quad;
use super::super::probe::Trace;
use super::super::select::Areas;
use super::{Completed, InkColours, STRAY, complete};

const PAPER: [u8; 3] = [200, 205, 210];
const INK: [u8; 3] = [30, 30, 35];
/// A slightly lighter ink, as an anti-aliased or faded stroke end: another cluster's centre
/// would miss it, the measured colour does not.
const FADED: [u8; 3] = [40, 38, 45];

/// A 200 × 120 plate at the frame origin with the analysis window at (50, 40) 100 × 40, the
/// quad just inside it and a line height of 30.
fn areas() -> Areas {
    let window = PixelRect {
        x: 50,
        y: 40,
        width: 100,
        height: 40,
    };
    Areas {
        quad: rect_quad(54.0, 44.0, 146.0, 76.0),
        ruby: Vec::new(),
        window,
        analysis: window,
        plate: PixelRect {
            x: 0,
            y: 0,
            width: 200,
            height: 120,
        },
        line_height: 30.0,
        refit: false,
    }
}

fn clusters() -> Clusters {
    Clusters {
        centres: vec![lab(PAPER), lab(INK)],
        labels: Vec::new(),
        counts: vec![1, 1],
        spread: vec![1.0, 1.0],
        cost: 0.0,
    }
}

fn style() -> LetteringStyle {
    LetteringStyle {
        fill_rgb: INK,
        outline_rgb: None,
        outline_px: 0.0,
        soft_outline: false,
        stroke_px: 4.0,
        line_height_px: 30.0,
    }
}

/// A stroke's rectangle (left, top, right, bottom; exclusive), colour, and whether segmentation
/// found it.
type Stroke = ((u32, u32, u32, u32), [u8; 3], bool);

/// Paint `strokes` in their colours over paper, and mask the ones segmentation found.
fn scene(strokes: &[Stroke]) -> (RgbImage, GrayImage) {
    let inside =
        |x: u32, y: u32, (l, t, r, b): (u32, u32, u32, u32)| x >= l && x < r && y >= t && y < b;
    let image = RgbImage::from_fn(200, 120, |x, y| {
        Rgb(strokes
            .iter()
            .find(|(rect, _, _)| inside(x, y, *rect))
            .map_or(PAPER, |(_, colour, _)| *colour))
    });
    let mask = GrayImage::from_fn(200, 120, |x, y| {
        let set = strokes
            .iter()
            .any(|(rect, _, segmented)| *segmented && inside(x, y, *rect));
        Luma([if set { 255 } else { 0 }])
    });
    (image, mask)
}

fn run(image: &RgbImage, mask: GrayImage, trace: &mut Trace) -> Result<Completed, &'static str> {
    let clusters = clusters();
    let ink = [false, true];
    let style = style();
    let colours = InkColours {
        clusters: &clusters,
        ink: &ink,
        style: &style,
    };
    complete(image, mask, &areas(), &colours, 2, trace)
}

#[test]
fn a_clipped_stroke_end_joins_the_mask() {
    // The segmented stroke runs out of the window's right edge at 150; its faded end continues
    // to 160, within half a line.
    let (image, mask) = scene(&[
        ((80, 55, 150, 60), INK, true),
        ((150, 55, 160, 60), FADED, false),
    ]);
    let mut trace = Trace::default();
    let completed = run(&image, mask, &mut trace).expect("complete");
    assert_eq!(completed.mask.get_pixel(158, 57).0[0], 255);
    let counts = trace.completeness.expect("counts");
    assert_eq!(counts.joined, 50);
    assert_eq!(counts.stray, 0);
}

#[test]
fn ink_coloured_picture_running_past_the_region_stays() {
    // A dark rail touching the stroke and running to the plate's edge is picture.
    let (image, mask) = scene(&[
        ((80, 55, 120, 60), INK, true),
        ((120, 57, 200, 59), INK, false),
    ]);
    let mut trace = Trace::default();
    let completed = run(&image, mask, &mut trace).expect("complete");
    assert_eq!(completed.mask.get_pixel(190, 58).0[0], 0);
    assert_eq!(trace.completeness.expect("counts").joined, 0);
}

#[test]
fn strokes_apart_from_the_mask_inside_the_quad_fall_back() {
    let (image, mask) = scene(&[
        ((60, 50, 70, 70), INK, true),
        ((100, 48, 130, 72), INK, false),
    ]);
    let mut trace = Trace::default();
    assert_eq!(run(&image, mask, &mut trace).err(), Some(STRAY));
    let counts = trace.completeness.expect("counts");
    assert_eq!(counts.stray, 30 * 24);
}

#[test]
fn a_few_stray_specks_are_tolerated() {
    let (image, mask) = scene(&[
        ((60, 50, 120, 70), INK, true),
        ((130, 50, 132, 52), INK, false),
    ]);
    let mut trace = Trace::default();
    run(&image, mask, &mut trace).expect("complete");
    assert_eq!(trace.completeness.expect("counts").stray, 4);
}

#[test]
fn a_loose_box_is_refitted_to_the_ink_and_needs_to_hold_it() {
    let (image, mask) = scene(&[((60, 50, 140, 70), INK, true)]);
    let clusters = clusters();
    let ink = [false, true];
    let style = style();
    let colours = InkColours {
        clusters: &clusters,
        ink: &ink,
        style: &style,
    };
    let mut loose = areas();
    loose.refit = true;
    let completed = complete(
        &image,
        mask.clone(),
        &loose,
        &colours,
        2,
        &mut Trace::default(),
    )
    .unwrap();
    let (l, t, r, b) = completed.lettering.expect("refitted").bounds();
    assert_eq!((l, t, r, b), (60.0, 50.0, 140.0, 70.0));

    // Ink filling a fifth of the box is not what the box marks.
    let (image, mask) = scene(&[((60, 50, 80, 70), INK, true)]);
    let outcome = complete(&image, mask, &loose, &colours, 2, &mut Trace::default());
    assert_eq!(outcome.err(), Some(super::UNSEPARATED));
}

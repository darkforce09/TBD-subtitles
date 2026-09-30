use job_model::onscreen::LetteringStyle;

use super::*;

/// Every character advances 0.6 em, scaled by the width axis.
pub(crate) struct FixedMetrics;

impl Metrics for FixedMetrics {
    fn line_width(&self, line: &str, size: f64, width_axis: f64) -> f64 {
        line.chars().count() as f64 * 0.6 * size * width_axis / 100.0
    }

    fn cap_ratio(&self) -> f64 {
        0.7
    }

    fn descent_ratio(&self) -> f64 {
        0.25
    }
}

fn style(stroke: f64, line_height: f64) -> LetteringStyle {
    LetteringStyle {
        fill_rgb: [255, 255, 255],
        outline_rgb: None,
        outline_px: 0.0,
        soft_outline: false,
        stroke_px: stroke,
        line_height_px: line_height,
    }
}

#[test]
fn weight_follows_the_stroke_to_height_ratio() {
    assert_eq!(weight(&style(5.0, 100.0)), 400.0);
    assert_eq!(weight(&style(8.0, 100.0)), 600.0);
    assert_eq!(weight(&style(11.0, 100.0)), 600.0);
    assert_eq!(weight(&style(12.0, 100.0)), 700.0);
    assert_eq!(weight(&style(15.0, 100.0)), 700.0);
    assert_eq!(weight(&style(16.0, 100.0)), 900.0);
    assert_eq!(weight(&style(30.0, 100.0)), 900.0);
    assert_eq!(weight(&style(3.0, 0.0)), 400.0);
    assert!((target_cap(&style(5.0, 40.0)) - 28.0).abs() < 1e-9);
}

#[test]
fn a_narrower_width_is_tried_before_a_smaller_size() {
    // Cap 7 → 10 px per em; ten characters are 60 px wide at 100, 52.5 at 87.5, 45 at 75.
    let english = "ABCDEFGHIJ";
    let at_87 = Area {
        width: 60.0,
        height: 20.0,
    };
    let layout = fit(&FixedMetrics, english, at_87, 7.0, 1.0).unwrap();
    assert_eq!(layout.cap, 7.0);
    assert_eq!(layout.width_axis, 87.5);
    assert_eq!(layout.lines, vec![english.to_string()]);

    let at_75 = Area {
        width: 50.0,
        height: 20.0,
    };
    let layout = fit(&FixedMetrics, english, at_75, 7.0, 1.0).unwrap();
    assert_eq!(layout.cap, 7.0);
    assert_eq!(layout.width_axis, 75.0);

    let smaller = Area {
        width: 40.0,
        height: 20.0,
    };
    let layout = fit(&FixedMetrics, english, smaller, 7.0, 1.0).unwrap();
    assert!(layout.cap < 7.0);
    assert!(FixedMetrics.line_width(english, layout.size, layout.width_axis) <= 0.92 * 40.0);
}

#[test]
fn the_size_never_exceeds_the_target() {
    let roomy = Area {
        width: 1000.0,
        height: 500.0,
    };
    let layout = fit(&FixedMetrics, "SALE", roomy, 20.0, 5.0).unwrap();
    assert_eq!(layout.cap, 20.0);
    assert_eq!(layout.width_axis, 100.0);
}

#[test]
fn greedy_wrapping_fills_lines_and_keeps_explicit_breaks() {
    let lines = wrap(&FixedMetrics, "AB CD EF\nGH", 6.0 * 5.0, 10.0, 100.0, false).unwrap();
    assert_eq!(lines, vec!["AB CD", "EF", "GH"]);
    assert!(wrap(&FixedMetrics, "ABCDEFGHIJ", 30.0, 10.0, 100.0, false).is_none());
}

#[test]
fn vertical_areas_stack_one_word_per_line() {
    let tall = Area {
        width: 40.0,
        height: 200.0,
    };
    assert!(tall.vertical());
    let layout = fit(&FixedMetrics, "GO BIG", tall, 7.0, 1.0).unwrap();
    assert_eq!(layout.lines, vec!["GO", "BIG"]);

    let wide = Area {
        width: 40.0,
        height: 20.0,
    };
    assert!(!wide.vertical());
    let layout = fit(&FixedMetrics, "GO BIG", wide, 7.0, 1.0).unwrap();
    assert_eq!(layout.lines, vec!["GO BIG"]);
}

#[test]
fn nothing_fits_below_the_readable_size() {
    let tiny = Area {
        width: 30.0,
        height: 10.0,
    };
    assert!(fit(&FixedMetrics, "A VERY LONG SENTENCE INDEED", tiny, 7.0, 5.0).is_none());
    assert!(fit(&FixedMetrics, "OK", tiny, 0.0, 5.0).is_none());
    // Writing whose own size is below the readable minimum never letters smaller.
    let roomy = Area {
        width: 500.0,
        height: 100.0,
    };
    assert!(fit(&FixedMetrics, "OK", roomy, 4.0, 5.0).is_none());
}

#[test]
fn the_readable_minimum_itself_is_tried() {
    // "ABCDE" at cap c is 5 × 0.6 × c / 0.7 wide at 75: fits 0.92 × 20 when c ≤ 5.72.
    let area = Area {
        width: 20.0,
        height: 50.0,
    };
    let layout = fit(&FixedMetrics, "ABCDE", area, 20.0, 5.7).unwrap();
    assert!(layout.cap >= 5.7 && layout.cap <= 5.72 + 1e-9);
}

#[test]
fn lines_are_centred_and_the_block_is_centred_vertically() {
    let area = Area {
        width: 100.0,
        height: 60.0,
    };
    let layout = Layout {
        lines: vec!["ABCD".into(), "AB".into()],
        size: 10.0,
        cap: 7.0,
        width_axis: 100.0,
    };
    let placed = placements(&FixedMetrics, &layout, area);
    assert_eq!(placed.len(), 2);
    assert!((placed[0].x - (100.0 - 24.0) / 2.0).abs() < 1e-9);
    assert!((placed[1].x - (100.0 - 12.0) / 2.0).abs() < 1e-9);
    let block = block_height(&FixedMetrics, 2, 10.0);
    assert!((block - (11.5 + 9.5)).abs() < 1e-9);
    let top = (60.0 - block) / 2.0;
    assert!((placed[0].baseline - (top + 7.0)).abs() < 1e-9);
    assert!((placed[1].baseline - placed[0].baseline - 11.5).abs() < 1e-9);
}

#[test]
fn a_container_keeps_the_source_size_ratio() {
    // A small caption (line height 20) crowded in a narrow box beside a large title (line
    // height 40) in a roomy box: the caption limits the shared factor.
    let caption = (
        "NOTICE BOARD",
        Area {
            width: 50.0,
            height: 30.0,
        },
        0.7 * 20.0,
    );
    let title = (
        "SHOP",
        Area {
            width: 400.0,
            height: 80.0,
        },
        0.7 * 40.0,
    );
    let best: Vec<(f64, f64)> = [caption, title]
        .iter()
        .map(|(english, area, target)| {
            let layout = fit(&FixedMetrics, english, *area, *target, 1.0).unwrap();
            (*target, layout.cap)
        })
        .collect();
    assert!(best[0].1 < best[0].0);
    assert_eq!(best[1].1, best[1].0);
    let k = shared_scale(&best);
    assert!(k < 1.0);
    let small = fit_at(&FixedMetrics, caption.0, caption.1, caption.2 * k).unwrap();
    let large = fit_at(&FixedMetrics, title.0, title.1, title.2 * k).unwrap();
    assert!((large.cap / small.cap - 2.0).abs() < 1e-9);
    assert_eq!(shared_scale(&[(10.0, 8.0), (20.0, 20.0)]), 0.8);
    assert_eq!(shared_scale(&[]), 1.0);
}

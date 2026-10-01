use super::*;
use job_model::onscreen::Point;

#[test]
fn a_proxy_box_scales_by_three_from_640_by_360_to_1080p() {
    let found: Quad = [(100.0, 50.0), (220.5, 50.0), (220.5, 80.0), (100.0, 80.0)];
    assert_eq!(
        scale_quad(found, (640, 360), (1920, 1080)),
        [
            (300.0, 150.0),
            (661.5, 150.0),
            (661.5, 240.0),
            (300.0, 240.0)
        ]
    );
}

#[test]
fn a_scaled_box_reaching_past_an_edge_is_clipped_to_the_last_pixel() {
    let found: Quad = [(-2.0, 340.0), (645.0, 340.0), (645.0, 365.0), (-2.0, 365.0)];
    assert_eq!(
        scale_quad(found, (640, 360), (1920, 1080)),
        [
            (0.0, 1020.0),
            (1919.0, 1020.0),
            (1919.0, 1079.0),
            (0.0, 1079.0)
        ]
    );
}

#[test]
fn a_non_uniform_proxy_scales_each_axis_on_its_own() {
    let found: Quad = [(64.0, 36.0); 4];
    assert_eq!(
        scale_quad(found, (640, 362), (1440, 817))[0],
        (144.0, 36.0 * 817.0 / 362.0)
    );
}

#[test]
fn a_full_resolution_box_inside_the_frame_keeps_its_corners() {
    let found: Quad = [(10.0, 20.0), (300.0, 18.5), (301.0, 64.0), (11.0, 66.0)];
    assert_eq!(clip_to(found, (1920, 1080)), found);
    assert_eq!(
        clip_to([(1920.0, 1080.0); 4], (1920, 1080)),
        [(1919.0, 1079.0); 4]
    );
}

#[test]
fn pool_regions_become_drawing_corners() {
    let quad = job_model::onscreen::Quad([
        Point { x: 1.5, y: 2.0 },
        Point { x: 9.0, y: 2.0 },
        Point { x: 9.0, y: 7.25 },
        Point { x: 1.5, y: 7.25 },
    ]);
    assert_eq!(
        corners(&quad),
        [(1.5, 2.0), (9.0, 2.0), (9.0, 7.25), (1.5, 7.25)]
    );
}

#[test]
fn picks_spread_over_the_clip_and_never_repeat() {
    assert_eq!(pick(240, 6), vec![0, 40, 80, 120, 160, 200]);
    assert_eq!(pick(3, 6), vec![0, 1, 2]);
    assert_eq!(pick(7, 3), vec![0, 2, 4]);
    assert!(pick(0, 6).is_empty());
    assert!(pick(240, 0).is_empty());
}

#[test]
fn a_drawn_box_colours_its_edges_and_leaves_its_inside() {
    let mut image = RgbImage::new(20, 20);
    let quad: Quad = [(5.0, 5.0), (14.0, 5.0), (14.0, 14.0), (5.0, 14.0)];
    draw_quad(&mut image, &quad, PROXY_COLOUR, 1);
    assert_eq!(*image.get_pixel(5, 5), PROXY_COLOUR);
    assert_eq!(*image.get_pixel(10, 5), PROXY_COLOUR);
    assert_eq!(*image.get_pixel(14, 10), PROXY_COLOUR);
    assert_eq!(*image.get_pixel(10, 10), Rgb([0, 0, 0]));
    assert_eq!(*image.get_pixel(10, 6), Rgb([0, 0, 0]));
}

#[test]
fn a_thick_box_spreads_one_pixel_each_side_and_stays_inside_the_image() {
    let mut image = RgbImage::new(20, 20);
    let quad: Quad = [(0.0, 0.0), (19.0, 0.0), (19.0, 19.0), (0.0, 19.0)];
    draw_quad(&mut image, &quad, FULL_COLOUR, FULL_THICKNESS);
    assert_eq!(*image.get_pixel(10, 0), FULL_COLOUR);
    assert_eq!(*image.get_pixel(10, 1), FULL_COLOUR);
    assert_eq!(*image.get_pixel(10, 2), Rgb([0, 0, 0]));
    assert_eq!(*image.get_pixel(18, 10), FULL_COLOUR);
}

#[test]
fn the_legend_names_both_colours() {
    let legend = legend(true);
    assert!(legend.contains("green, 3 px: the production pool's boxes"));
    assert!(legend.contains("magenta, 1 px: the 640-wide proxy's boxes, scaled up"));
    assert!(super::legend(false).contains("no full-resolution boxes"));
}

#[test]
fn an_image_is_written_with_both_box_sets() {
    let dir = std::env::temp_dir().join(format!("detect-bench-overlay-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("a temporary folder");
    let frame = PaddedFrame {
        width: 64,
        height: 36,
        padded_height: 64,
        rgb: vec![0; 64 * 64 * 3],
    };
    let full = vec![(
        job_model::onscreen::Quad([
            Point { x: 4.0, y: 4.0 },
            Point { x: 30.0, y: 4.0 },
            Point { x: 30.0, y: 20.0 },
            Point { x: 4.0, y: 20.0 },
        ]),
        0.9,
    )];
    let proxy: Vec<Quad> = vec![[(20.0, 10.0), (30.0, 10.0), (30.0, 17.0), (20.0, 17.0)]];
    let path = dir.join("frame.png");
    write_image(&path, &frame, Some(&full), &proxy, (32, 18)).expect("the image is written");
    let written = image::open(&path).expect("a PNG").to_rgb8();
    std::fs::remove_dir_all(&dir).expect("the temporary folder is removed");
    assert_eq!(written.dimensions(), (64, 36));
    assert_eq!(*written.get_pixel(10, 4), FULL_COLOUR);
    assert_eq!(*written.get_pixel(50, 20), PROXY_COLOUR);
    assert_eq!(*written.get_pixel(50, 27), Rgb([0, 0, 0]));
}

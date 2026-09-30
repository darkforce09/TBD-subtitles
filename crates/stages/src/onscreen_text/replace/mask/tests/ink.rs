use super::super::cluster::{self, Clusters, lab};
use super::super::panel::panel;
use super::{lettering, near_segment, outlined_over_ring, without_blends};

/// A window of `w` × `h` painted by `paint`, split into `k` clusters.
fn window(w: u32, h: u32, k: usize, paint: impl Fn(u32, u32) -> [u8; 3]) -> Clusters {
    let pixels: Vec<_> = (0..h)
        .flat_map(|y| (0..w).map(move |x| (x, y)))
        .map(|(x, y)| lab(paint(x, y)))
        .collect();
    cluster::partitions(&pixels)
        .into_iter()
        .find(|c| c.centres.len() == k)
        .expect("a partition with k clusters")
}

fn cluster_of(clusters: &Clusters, rgb: [u8; 3]) -> usize {
    usize::from(cluster::nearest_centre(lab(rgb), &clusters.centres))
}

const BLUE: [u8; 3] = [60, 90, 150];
const WHITE: [u8; 3] = [245, 245, 245];
const BLACK: [u8; 3] = [15, 15, 20];

#[test]
fn a_colour_between_two_others_is_their_blend() {
    let (a, b) = (lab(WHITE), lab(BLUE));
    let middle: [f32; 3] = std::array::from_fn(|i| (a[i] + b[i]) / 2.0);
    assert!(near_segment(middle, a, b));
    assert!(!near_segment(lab(BLACK), a, b));
    assert!(!near_segment(middle, a, a));
}

#[test]
fn anti_aliasing_stays_ink_but_never_becomes_the_outline() {
    let blend = [150, 168, 198];
    let clusters = window(40, 20, 4, |x, _| match x {
        0..10 => WHITE,
        10..12 => blend,
        12..30 => BLUE,
        _ => BLACK,
    });
    let (white, grey, blue, black) = (
        cluster_of(&clusters, WHITE),
        cluster_of(&clusters, blend),
        cluster_of(&clusters, BLUE),
        cluster_of(&clusters, BLACK),
    );
    let mut core = vec![false; 4];
    let mut background = vec![false; 4];
    core[white] = true;
    core[grey] = true;
    core[black] = true;
    background[blue] = true;
    let kept = without_blends(&clusters, &core, &background);
    assert!(kept[white] && kept[black] && !kept[grey], "{kept:?}");
}

#[test]
fn outlined_letters_inside_the_box_are_ink_and_the_ring_colour_is_background() {
    let letter = |x: u32, y: u32| (10..30).contains(&x) && (8..22).contains(&y);
    let outline = |x: u32, y: u32| (8..32).contains(&x) && (6..24).contains(&y);
    let clusters = window(40, 30, 3, |x, y| {
        if letter(x, y) && !(14..26).contains(&x) {
            WHITE
        } else if outline(x, y) {
            BLACK
        } else {
            BLUE
        }
    });
    let inside = vec![true; 40 * 30];
    let selection = lettering(&clusters, 40, 30, &inside, 2).expect("lettering");
    let (white, blue, black) = (
        cluster_of(&clusters, WHITE),
        cluster_of(&clusters, BLUE),
        cluster_of(&clusters, BLACK),
    );
    assert!(selection.background[blue] && !selection.ink[blue]);
    assert!(selection.core[white] && selection.core[black]);
    assert_eq!(selection.panel, None);
}

#[test]
fn a_colour_filling_the_box_that_the_ring_misses_is_a_panel() {
    let sign = |x: u32, y: u32| (4..36).contains(&x) && (4..26).contains(&y);
    let digit = |x: u32, y: u32| (12..16).contains(&x) && (8..22).contains(&y);
    let paint = |x, y| {
        if digit(x, y) {
            BLACK
        } else if sign(x, y) {
            WHITE
        } else {
            BLACK
        }
    };
    let clusters = window(40, 30, 2, paint);
    let inside: Vec<bool> = (0..30u32)
        .flat_map(|y| (0..40u32).map(move |x| sign(x, y)))
        .collect();
    assert!(lettering(&clusters, 40, 30, &inside, 2).is_none());
    let selection = panel(&clusters, 40, 30, &inside).expect("a panel");
    let (white, black) = (cluster_of(&clusters, WHITE), cluster_of(&clusters, BLACK));
    assert!(selection.background[white] && selection.ink[black]);
    assert_eq!(selection.panel, Some((4, 4, 36, 26)));
}

#[test]
fn a_panel_split_into_pieces_is_not_a_sign() {
    let clusters = window(40, 30, 2, |x, _| if x % 8 < 5 { WHITE } else { BLACK });
    let inside = vec![true; 40 * 30];
    assert!(panel(&clusters, 40, 30, &inside).is_none());
}

/// A 300 × 100 window whose top and bottom ring rows run through the fill and outline of a line
/// above and below, with outlined lettering of `fill` and `outline` in the middle over `picture`,
/// and the quad's pixels (rows 36 to 64) flagged: the picture covers two fifths of the quad but
/// shows in the ring only along its sides.
fn ring_crossed(
    fill: impl Fn(u32, u32) -> [u8; 3],
    outline: [u8; 3],
    picture: [u8; 3],
) -> (Clusters, Vec<bool>) {
    let paint = |x: u32, y: u32| {
        let glyph_fill = (13..287).contains(&x) && (44..56).contains(&y);
        let glyph_outline = (10..290).contains(&x) && (41..59).contains(&y);
        match y {
            0..2 | 98.. => fill(x, y),
            2 | 97 => outline,
            _ if glyph_fill => fill(x, y),
            _ if glyph_outline => outline,
            _ => picture,
        }
    };
    let inside = (0..100u32)
        .flat_map(|y| (0..300u32).map(move |x| (4..296).contains(&x) && (36..64).contains(&y)))
        .collect();
    (window(300, 100, 3, paint), inside)
}

const RED: [u8; 3] = [200, 40, 90];

#[test]
fn a_picture_colour_the_crossing_lettering_crowds_out_of_the_ring_is_background() {
    let (clusters, inside) = ring_crossed(|_, _| WHITE, BLACK, RED);
    let selection = outlined_over_ring(&clusters, 300, 100, &inside, 6).expect("ring lettering");
    let (white, red, black) = (
        cluster_of(&clusters, WHITE),
        cluster_of(&clusters, RED),
        cluster_of(&clusters, BLACK),
    );
    assert!(
        selection.background[red] && !selection.ink[red],
        "{selection:?}"
    );
    assert!(
        selection.ink[white] && selection.ink[black],
        "{selection:?}"
    );
    assert!(selection.outlined);
}

#[test]
fn a_ring_fill_may_spread_with_the_contrast_of_its_outline() {
    let grey = [160, 160, 160];
    let checker = move |x: u32, y: u32| {
        if (x + y).is_multiple_of(2) {
            WHITE
        } else {
            grey
        }
    };
    let (clusters, inside) = ring_crossed(checker, BLACK, RED);
    let (fill, black) = (cluster_of(&clusters, WHITE), cluster_of(&clusters, BLACK));
    assert_eq!(fill, cluster_of(&clusters, grey));
    let contrast = cluster::distance(clusters.centres[fill], clusters.centres[black]);
    assert!(
        clusters.spread[fill] > 15.0 && clusters.spread[fill] <= 0.2 * contrast,
        "spread {} against contrast {contrast}",
        clusters.spread[fill]
    );
    assert!(outlined_over_ring(&clusters, 300, 100, &inside, 6).is_some());

    let dim = [60, 60, 70];
    let (clusters, inside) = ring_crossed(checker, dim, RED);
    let (fill, outline) = (cluster_of(&clusters, WHITE), cluster_of(&clusters, dim));
    assert_eq!(fill, cluster_of(&clusters, grey));
    let contrast = cluster::distance(clusters.centres[fill], clusters.centres[outline]);
    assert!(clusters.spread[fill] > 15.0_f32.max(0.2 * contrast));
    assert!(outlined_over_ring(&clusters, 300, 100, &inside, 6).is_none());
}

use super::super::cluster::{self, Clusters, lab};
use super::{lettering, near_segment, panel, without_blends};

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

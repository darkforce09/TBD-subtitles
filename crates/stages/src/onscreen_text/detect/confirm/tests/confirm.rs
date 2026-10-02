use std::sync::Mutex;

use job_model::outputs::VideoStream;

use super::*;
use crate::onscreen_text::detect::fixtures::{
    Answer, EXACT_OFFSET, Pool, SIZE, Source, Temporary, Writing, region, stream, timeline,
    yuv_frame,
};
use crate::onscreen_text::detect::regions::{Observation, append_observation, start_occurrence};

/// Each occurrence's keyframe, in occurrence order; frame 105 shows nothing and one occurrence
/// has no keyframe.
const KEYFRAMES: [Option<u64>; 13] = [
    Some(50),
    Some(10),
    Some(30),
    Some(30),
    Some(20),
    Some(70),
    Some(40),
    Some(60),
    Some(80),
    Some(105),
    None,
    Some(90),
    Some(100),
];

/// One writing per tenth frame, each with its own ink, all in the same place.
fn writings() -> Vec<Writing> {
    (1..=10)
        .map(|n| Writing {
            rect: (16, 12, 80, 20),
            frames: n * 10..=n * 10,
            ink: n as u8 * 20 + 5,
            paper: 250,
        })
        .collect()
}

fn ink(index: u64) -> u8 {
    (index / 10) as u8 * 20 + 5
}

/// The scan's closed document: one single-frame occurrence per keyframe, with the even frames
/// held in memory.
fn closed(writings: &[Writing]) -> Closed {
    let frames = timeline(120);
    let mut document = TextDocument {
        width: SIZE.0,
        height: SIZE.1,
        decoded_frames: 120,
        ..TextDocument::default()
    };
    let quad = region(&writings[0], 0.0);
    for keyframe in KEYFRAMES {
        let (time_s, end_s) = frames[keyframe.unwrap_or(0) as usize];
        let occurrence = start_occurrence(&mut document, time_s, 0.9);
        let observation = Observation {
            quad,
            confidence: 0.9,
            surface_rgb: None,
        };
        append_observation(
            &mut document.occurrences[occurrence],
            &observation,
            time_s,
            end_s,
        );
    }
    let held = [20u64, 40, 60, 80, 100]
        .into_iter()
        .map(|index| (index, Arc::new(yuv_frame(writings, index, false, &frames))))
        .collect();
    Closed {
        document,
        keyframes: KEYFRAMES
            .iter()
            .map(|keyframe| keyframe.map(|index| (0, index)))
            .collect(),
        frames: held,
        min_confirm_frames: 1,
    }
}

fn colour(stream: &VideoStream) -> Coefficients {
    Coefficients::of(stream)
}

#[test]
fn keyframes_are_confirmed_in_occurrence_order_from_memory_or_from_the_video() {
    let writings = writings();
    let root = Temporary::new("confirm");
    std::fs::create_dir_all(root.0.join("visual/crops")).unwrap();
    std::fs::write(root.0.join("visual/crops/stale.png"), b"old").unwrap();
    let mut source = Source::new(writings.clone(), 120);
    source.finished = true;
    let mut pool = Pool::new(writings.clone(), 2, Answer::Oldest);
    let mut stats = ScanStats::default();
    let calls = Mutex::new(Vec::new());
    let progress = |done: usize, total: usize| calls.lock().unwrap().push((done, total));
    let document = confirm_keyframes(
        closed(&writings),
        &colour(&stream()),
        &mut source,
        &mut pool,
        &root.0,
        &mut stats,
        &progress,
    )
    .unwrap();
    let distinct = [50, 10, 30, 20, 70, 40, 60, 80, 105, 90, 100];
    let inks: Vec<Vec<u8>> = distinct
        .iter()
        .map(|&index| {
            if index == 105 {
                Vec::new()
            } else {
                vec![ink(index)]
            }
        })
        .collect();
    assert_eq!(pool.confirmations, [inks[..8].to_vec(), inks[8..].to_vec()]);
    assert_eq!(source.stills, [vec![50, 10, 30, 70], vec![105, 90]]);
    assert_eq!(
        (stats.keyframes_from_ram, stats.keyframes_from_ffmpeg),
        (5, 6)
    );
    let kept: Vec<&str> = document
        .occurrences
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(
        kept,
        [
            "text-000001",
            "text-000002",
            "text-000003",
            "text-000004",
            "text-000005",
            "text-000006",
            "text-000007",
            "text-000008",
            "text-000009",
            "text-000012",
            "text-000013"
        ],
        "the blank keyframe and the occurrence without one leave"
    );
    for item in &document.occurrences {
        assert_eq!(item.frames[0].quad, region(&writings[0], EXACT_OFFSET));
        assert!(root.0.join(&item.crops[0]).is_file());
        assert!(
            root.0
                .join(&item.keyframe.as_ref().unwrap().image)
                .is_file()
        );
    }
    let mut keyframe_files: Vec<String> = std::fs::read_dir(root.0.join("visual/keyframes"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    keyframe_files.sort();
    assert_eq!(keyframe_files.len(), 10, "one image per confirmed keyframe");
    assert!(
        !root.0.join("visual/crops/stale.png").exists(),
        "stale files go"
    );
    let calls = calls.into_inner().unwrap();
    assert_eq!(calls.len(), 11);
    assert_eq!(calls.last(), Some(&(131, 131)));
}

#[test]
fn a_keyframe_from_memory_and_from_the_video_give_the_same_files() {
    let writings = writings();
    let run = |hold: bool| {
        let root = Temporary::new("confirm-same");
        let mut source = Source::new(writings.clone(), 120);
        source.finished = true;
        let mut pool = Pool::new(writings.clone(), 1, Answer::Oldest);
        let mut closed = closed(&writings);
        if !hold {
            closed.frames.clear();
        }
        let document = confirm_keyframes(
            closed,
            &colour(&stream()),
            &mut source,
            &mut pool,
            &root.0,
            &mut ScanStats::default(),
            &|_, _| {},
        )
        .unwrap();
        let files: Vec<Vec<u8>> = document
            .occurrences
            .iter()
            .flat_map(|item| [item.crops[0].clone(), item.keyframe.clone().unwrap().image])
            .map(|path| std::fs::read(root.0.join(path)).unwrap())
            .collect();
        (document, files)
    };
    assert_eq!(run(true), run(false));
}

#[test]
fn non_persistent_keyframes_are_not_confirmed_unless_shared_with_persistent_occurrence() {
    let writings = writings();
    let root = Temporary::new("confirm-targeted");
    let mut source = Source::new(writings.clone(), 120);
    source.finished = true;
    let mut pool = Pool::new(writings.clone(), 1, Answer::Oldest);
    let frames = timeline(120);
    let mut document = TextDocument {
        width: SIZE.0,
        height: SIZE.1,
        decoded_frames: 120,
        ..TextDocument::default()
    };
    let quad = region(&writings[0], 0.0);
    // Occurrence 0: persistent (5 frames), keyframe at index 20
    let occ0 = start_occurrence(&mut document, frames[20].0, 0.9);
    for &(time_s, end_s) in &frames[18..=22] {
        append_observation(
            &mut document.occurrences[occ0],
            &Observation {
                quad,
                confidence: 0.9,
                surface_rgb: None,
            },
            time_s,
            end_s,
        );
    }
    // Occurrence 1: short (1 frame), shares keyframe at index 20
    let occ1 = start_occurrence(&mut document, frames[20].0, 0.9);
    append_observation(
        &mut document.occurrences[occ1],
        &Observation {
            quad,
            confidence: 0.9,
            surface_rgb: None,
        },
        frames[20].0,
        frames[20].1,
    );
    // Occurrence 2: short (1 frame), isolated keyframe at index 40 (non-persistent)
    let occ2 = start_occurrence(&mut document, frames[40].0, 0.9);
    append_observation(
        &mut document.occurrences[occ2],
        &Observation {
            quad,
            confidence: 0.9,
            surface_rgb: None,
        },
        frames[40].0,
        frames[40].1,
    );
    let held = [
        (20u64, Arc::new(yuv_frame(&writings, 20, false, &frames))),
        (40u64, Arc::new(yuv_frame(&writings, 40, false, &frames))),
    ]
    .into_iter()
    .collect();
    let closed = Closed {
        document,
        keyframes: vec![Some((0, 20)), Some((0, 20)), Some((0, 40))],
        frames: held,
        min_confirm_frames: 5,
    };
    let result = confirm_keyframes(
        closed,
        &colour(&stream()),
        &mut source,
        &mut pool,
        &root.0,
        &mut ScanStats::default(),
        &|_, _| {},
    )
    .unwrap();
    // Only keyframe 20 was confirmed; occurrence 0 and co-occurring 1 survived, isolated 2 left
    let kept_ids: Vec<&str> = result
        .occurrences
        .iter()
        .map(|item| item.id.as_str())
        .collect();
    assert_eq!(kept_ids, ["text-000001", "text-000002"]);
    // Server detector only received 1 keyframe (index 20), never index 40
    assert_eq!(pool.confirmations.len(), 1);
    assert_eq!(pool.confirmations[0].len(), 1);
}

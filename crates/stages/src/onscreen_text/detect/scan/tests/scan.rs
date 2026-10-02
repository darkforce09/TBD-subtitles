use std::path::PathBuf;
use std::sync::Mutex;

use inference::ocr::pool::Priority;
use job_model::onscreen::{TextKeyframe, TextOccurrence};
use job_model::outputs::ShotCut;

use super::*;
use crate::onscreen_text::detect::fixtures::{
    Answer, EXACT_OFFSET, Pool, SIZE, Source, Temporary, Writing, region, stream, timeline, writing,
};

/// How a scripted scan runs: the sessions, the order they answer in and the candidates' budget.
#[derive(Clone, Copy)]
struct Setup {
    sessions: usize,
    answer: Answer,
    budget: usize,
    flicker: bool,
    min_confirm_frames: usize,
    min_confirm_confidence: f64,
    min_bisection_samples: usize,
}

impl Default for Setup {
    fn default() -> Self {
        Self {
            sessions: 2,
            answer: Answer::Oldest,
            budget: ScanLimits::default().candidate_budget,
            flicker: false,
            min_confirm_frames: 1,
            min_confirm_confidence: 0.0,
            min_bisection_samples: 2,
        }
    }
}

struct Run {
    document: TextDocument,
    stats: ScanStats,
    timeline: Vec<(f64, f64)>,
    source: Source,
    pool: Pool,
    progress: Vec<(usize, usize)>,
    root: Temporary,
}

impl Run {
    fn new(writings: Vec<Writing>, count: u64, cuts: ShotChanges) -> Self {
        Self::with(writings, count, cuts, Setup::default())
    }

    fn with(writings: Vec<Writing>, count: u64, cuts: ShotChanges, setup: Setup) -> Self {
        let mut source = Source::new(writings.clone(), count);
        source.flicker = setup.flicker;
        let mut pool = Pool::new(writings, setup.sessions, setup.answer);
        let root = Temporary::new("scan");
        let calls = Mutex::new(Vec::new());
        let progress = |done: usize, total: usize| calls.lock().unwrap().push((done, total));
        let limits = ScanLimits {
            candidate_budget: setup.budget,
            min_confirm_frames: setup.min_confirm_frames,
            min_confirm_confidence: setup.min_confirm_confidence,
            min_bisection_samples: setup.min_bisection_samples,
        };
        let (document, stats) = run(
            &mut source,
            &stream(),
            &cuts,
            &root.0,
            &mut pool,
            &progress,
            limits,
        )
        .unwrap();
        Self {
            document,
            stats,
            timeline: timeline(count),
            source,
            pool,
            progress: calls.into_inner().unwrap(),
            root,
        }
    }

    fn time(&self, index: usize) -> f64 {
        self.timeline[index].0
    }

    fn frame_times(&self, item: &TextOccurrence) -> Vec<f64> {
        item.frames.iter().map(|frame| frame.time_s).collect()
    }

    fn times(&self, indices: &[usize]) -> Vec<f64> {
        indices.iter().map(|&index| self.time(index)).collect()
    }

    fn files(&self, folder: &str) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.root.0.join(folder))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    /// Every crop and keyframe file with its bytes.
    fn pngs(&self) -> Vec<(String, Vec<u8>)> {
        ["visual/crops", "visual/keyframes"]
            .iter()
            .flat_map(|folder| {
                self.files(folder).into_iter().map(move |name| {
                    let path = self.root.0.join(folder).join(&name);
                    (format!("{folder}/{name}"), std::fs::read(path).unwrap())
                })
            })
            .collect()
    }
}

/// The reader's and resume check's needs: exact bounds, tiled frames, a crop and a keyframe.
fn assert_complete(run: &Run, item: &TextOccurrence) {
    assert_eq!(item.frames.first().unwrap().time_s, item.start_s);
    assert_eq!(item.frames.last().unwrap().end_s, item.end_s);
    for frame in &item.frames {
        assert!(frame.end_s > frame.time_s && frame.quad.valid());
    }
    for pair in item.frames.windows(2) {
        assert_eq!(pair[1].time_s, pair[0].end_s, "frames tile");
    }
    assert_eq!(item.crops.len(), 1);
    let keyframe = item.keyframe.as_ref().unwrap();
    for path in [&item.crops[0], &keyframe.image] {
        let size = std::fs::metadata(run.root.0.join(path)).unwrap().len();
        assert!(size > 0, "{} is empty", path.display());
    }
    assert!(
        item.frames
            .iter()
            .any(|frame| frame.time_s == keyframe.time_s)
    );
    let surface = item.frames[0].surface_rgb;
    assert!(item.frames.iter().all(|frame| frame.surface_rgb == surface));
}

/// Three writings, two of them overlapping in time, a cut, and a corner that changes on every
/// frame so every sample is screened.
fn busy() -> (Vec<Writing>, u64, ShotChanges) {
    let writings = vec![
        writing(30..=90, 20, 240),
        Writing {
            rect: (16, 44, 80, 20),
            frames: 50..=200,
            ink: 240,
            paper: 20,
        },
        writing(250..=330, 60, 200),
    ];
    let cuts = ShotChanges {
        cuts: vec![ShotCut {
            time_s: timeline(400)[150].0,
            score: 50.0,
        }],
    };
    (writings, 400, cuts)
}

#[test]
fn a_region_gets_its_exact_first_and_last_frame_one_crop_and_one_keyframe() {
    let run = Run::new(vec![writing(17..=41, 20, 240)], 80, ShotChanges::default());
    let document = &run.document;
    assert_eq!(document.sample_step, 12);
    assert_eq!(document.proxy_width, SIZE.0, "screened at full resolution");
    assert_eq!((document.width, document.height), SIZE);
    assert_eq!(document.decoded_frames, 80);
    assert_eq!(document.occurrences.len(), 1);
    let item = &document.occurrences[0];
    assert_eq!(item.start_s, run.time(17));
    assert_eq!(item.end_s, run.time(42));
    assert_eq!(run.frame_times(item), run.times(&[17, 24, 36]));
    assert_complete(&run, item);
    assert_eq!(
        item.keyframe,
        Some(TextKeyframe {
            time_s: run.time(24),
            image: PathBuf::from("visual/keyframes/frame-00000024.png"),
        })
    );
    let screening = region(&run.pool.writings[0], 0.0);
    assert_eq!(item.frames[0].quad, screening, "quads are in source pixels");
    assert_eq!(
        item.frames[1].quad,
        region(&run.pool.writings[0], EXACT_OFFSET)
    );
    assert_eq!(item.frames[2].quad, screening);
    assert!(item.warnings.is_empty());
    assert_eq!(run.files("visual/crops"), ["text-000001.png"]);
    assert_eq!(run.files("visual/keyframes"), ["frame-00000024.png"]);
    assert!(
        run.source.stills.is_empty(),
        "the keyframe comes from memory"
    );
    assert_eq!(run.pool.confirmations, [vec![vec![20]]]);
    assert!(
        run.pool.screened <= 3 + 2 * 4,
        "repeated samples reuse screens: {} images",
        run.pool.screened
    );
    assert_eq!(
        run.progress,
        [(24, 0), (48, 0), (72, 0), (80, 0), (81, 81)],
        "one report per 24 frames, at the last frame and per keyframe"
    );
}

#[test]
fn a_different_writing_in_the_same_place_starts_a_new_occurrence_at_its_exact_frame() {
    let run = Run::new(
        vec![writing(100..=139, 20, 240), writing(140..=200, 240, 20)],
        240,
        ShotChanges::default(),
    );
    let occurrences = &run.document.occurrences;
    assert_eq!(occurrences.len(), 2);
    let (first, second) = (&occurrences[0], &occurrences[1]);
    assert_eq!((first.start_s, first.end_s), (run.time(100), run.time(140)));
    assert_eq!(
        (second.start_s, second.end_s),
        (run.time(140), run.time(201))
    );
    assert_eq!(run.frame_times(first), run.times(&[100, 108, 120, 132]));
    assert_eq!(
        run.frame_times(second),
        run.times(&[140, 144, 156, 168, 180, 192])
    );
    for item in occurrences {
        assert_complete(&run, item);
    }
    assert_eq!(first.keyframe.as_ref().unwrap().time_s, run.time(120));
    assert_eq!(second.keyframe.as_ref().unwrap().time_s, run.time(168));
    assert_eq!(
        run.files("visual/keyframes"),
        ["frame-00000120.png", "frame-00000168.png"]
    );
}

#[test]
fn a_cut_splits_a_continuous_region_exactly_at_the_cut_frame() {
    let cuts = ShotChanges {
        cuts: vec![ShotCut {
            time_s: 60.0 / 24.0,
            score: 50.0,
        }],
    };
    let run = Run::new(vec![writing(30..=100, 20, 240)], 130, cuts);
    let occurrences = &run.document.occurrences;
    assert_eq!(occurrences.len(), 2);
    let (before, after) = (&occurrences[0], &occurrences[1]);
    assert_eq!((before.start_s, before.end_s), (run.time(30), run.time(60)));
    assert_eq!((after.start_s, after.end_s), (run.time(60), run.time(101)));
    assert_eq!(run.frame_times(before), run.times(&[30, 36, 48, 59]));
    assert_eq!(run.frame_times(after), run.times(&[60, 72, 84, 96]));
    for item in occurrences {
        assert_complete(&run, item);
    }
}

#[test]
fn writing_from_the_first_to_the_final_frame_spans_the_whole_timeline() {
    let run = Run::new(vec![writing(0..=79, 20, 240)], 80, ShotChanges::default());
    let item = &run.document.occurrences[0];
    assert_eq!(run.document.occurrences.len(), 1);
    assert_eq!((item.start_s, item.end_s), (0.0, run.timeline[79].1));
    assert_eq!(
        run.frame_times(item),
        run.times(&[0, 12, 24, 36, 48, 60, 72, 79])
    );
    assert_complete(&run, item);
}

#[test]
fn the_scanned_document_deserialises_with_its_keyframes_and_screened_width() {
    let run = Run::new(vec![writing(17..=41, 20, 240)], 80, ShotChanges::default());
    let json = serde_json::to_string(&run.document).unwrap();
    assert!(json.contains("\"proxy_width\":128"));
    assert!(json.contains("\"sample_step\":12"));
    assert!(json.contains("\"keyframe\":{"));
    let back: TextDocument = serde_json::from_str(&json).unwrap();
    assert_eq!((back.proxy_width, back.sample_step), (128, 12));
    assert_eq!(back.occurrences.len(), 1);
    let (item, original) = (&back.occurrences[0], &run.document.occurrences[0]);
    assert_eq!(item.id, original.id);
    assert_eq!(item.crops, original.crops);
    assert_eq!(item.frames.len(), original.frames.len());
    let (keyframe, expected) = (
        item.keyframe.as_ref().unwrap(),
        original.keyframe.as_ref().unwrap(),
    );
    assert_eq!(keyframe.image, expected.image);
    assert!((keyframe.time_s - expected.time_s).abs() < 1e-9);
}

#[test]
fn a_video_without_writing_screens_only_its_first_sample() {
    let run = Run::new(Vec::new(), 50, ShotChanges::default());
    assert!(run.document.occurrences.is_empty());
    assert_eq!(run.document.decoded_frames, 50);
    assert_eq!(run.pool.screened, 1, "every later sample repeats the first");
    assert!(run.pool.confirmations.is_empty());
    assert!(run.source.stills.is_empty());
    assert_eq!(run.progress, [(24, 0), (48, 0), (50, 0)]);
    assert_eq!(run.stats.frames_decoded, 50);
    assert_eq!(run.stats.frames_screened, 1);
}

#[test]
fn results_answered_out_of_order_are_applied_in_sample_order() {
    let (writings, count, cuts) = busy();
    let flicker = Setup {
        flicker: true,
        ..Setup::default()
    };
    let oldest = Run::with(writings.clone(), count, cuts.clone(), flicker);
    let newest = Run::with(
        writings,
        count,
        cuts,
        Setup {
            answer: Answer::Newest,
            ..flicker
        },
    );
    let screens: Vec<u64> = newest
        .pool
        .answered
        .iter()
        .filter(|answered| answered.priority == Priority::Screen)
        .map(|answered| answered.seq)
        .collect();
    assert!(
        screens.windows(2).any(|pair| pair[1] < pair[0]),
        "the scripted sessions answered out of order: {screens:?}"
    );
    assert_eq!(oldest.document.occurrences.len(), 4, "the cut splits one");
    assert_eq!(newest.document, oldest.document);
    assert_eq!(newest.pngs(), oldest.pngs());
}

#[test]
fn one_and_two_sessions_give_identical_documents_and_files() {
    let (writings, count, cuts) = busy();
    let one = Run::with(
        writings.clone(),
        count,
        cuts.clone(),
        Setup {
            sessions: 1,
            flicker: true,
            ..Setup::default()
        },
    );
    let two = Run::with(
        writings,
        count,
        cuts,
        Setup {
            sessions: 2,
            answer: Answer::SecondOldest,
            flicker: true,
            ..Setup::default()
        },
    );
    assert_eq!(one.document, two.document);
    assert_eq!(one.pngs(), two.pngs());
    assert_eq!(one.stats.frames_screened, two.stats.frames_screened);
    for item in &one.document.occurrences {
        assert_complete(&one, item);
    }
}

#[test]
fn probes_run_ahead_of_screening_jobs_still_waiting() {
    let (writings, count, cuts) = busy();
    let run = Run::with(
        writings,
        count,
        cuts,
        Setup {
            flicker: true,
            ..Setup::default()
        },
    );
    assert!(
        run.pool
            .answered
            .iter()
            .any(|answered| answered.priority == Priority::Probe && answered.screens_waiting > 0),
        "a probe overtook a waiting screening job"
    );
    assert_eq!(
        run.stats.frames_screened, run.pool.screened as u64,
        "samples and probes both count"
    );
    assert!(run.stats.frames_probed > 0);
}

#[test]
fn keyframes_beyond_the_candidate_budget_come_from_the_video_with_the_same_document() {
    let (writings, count, cuts) = busy();
    let held = Run::with(writings.clone(), count, cuts.clone(), Setup::default());
    let decoded = Run::with(
        writings,
        count,
        cuts,
        Setup {
            budget: 0,
            ..Setup::default()
        },
    );
    let keyframes = held.document.occurrences.len() as u64;
    assert!(held.source.stills.is_empty());
    assert_eq!(held.stats.keyframes_from_ram, keyframes);
    assert_eq!(held.stats.keyframes_from_ffmpeg, 0);
    assert!(held.stats.keyframes_held_peak_bytes > 0);
    assert_eq!(decoded.stats.keyframes_from_ram, 0);
    assert_eq!(decoded.stats.keyframes_from_ffmpeg, keyframes);
    assert_eq!(decoded.stats.keyframes_held_peak_bytes, 0);
    assert_eq!(decoded.source.stills.concat().len() as u64, keyframes);
    assert_eq!(decoded.document, held.document);
    assert_eq!(decoded.pngs(), held.pngs());
}

#[test]
fn the_stats_carry_the_sessions_start_up_and_notes() {
    let run = Run::new(vec![writing(17..=41, 20, 240)], 80, ShotChanges::default());
    assert_eq!(run.stats.warmup_s, 1.5);
    assert_eq!(run.stats.engine_build_s, 0.0);
    assert_eq!(
        run.stats.notes.get("engine").map(String::as_str),
        Some("scripted")
    );
    assert_eq!(run.stats.frames_decoded, 80);
    assert_eq!(run.stats.frames_screened, run.pool.screened as u64);
}

#[test]
fn single_sample_transient_noise_runs_zero_probes_and_is_not_confirmed() {
    let run = Run::new(vec![writing(24..=24, 20, 240)], 80, ShotChanges::default());
    assert!(
        run.document.occurrences.is_empty(),
        "transient 1-frame noise is dropped"
    );
    assert_eq!(
        run.stats.frames_probed, 0,
        "lazy bisection runs zero probes on transient noise"
    );
    assert!(
        run.pool.confirmations.is_empty(),
        "transient noise never reaches server confirmation"
    );
}

#[test]
fn occurrences_under_min_confirm_frames_are_not_confirmed() {
    let run = Run::with(
        vec![writing(17..=41, 20, 240)],
        80,
        ShotChanges::default(),
        Setup {
            min_confirm_frames: 5,
            ..Setup::default()
        },
    );
    assert!(
        run.document.occurrences.is_empty(),
        "3-frame occurrence is under 5 frames and leaves without server confirmation"
    );
    assert!(
        run.pool.confirmations.is_empty(),
        "server confirmation was never invoked for non-targeted keyframes"
    );
}

#[test]
fn occurrences_under_min_confirm_confidence_are_not_confirmed() {
    let run = Run::with(
        vec![writing(17..=85, 20, 240)],
        100,
        ShotChanges::default(),
        Setup {
            min_confirm_frames: 1,
            min_confirm_confidence: 0.95,
            ..Setup::default()
        },
    );
    assert!(
        run.document.occurrences.is_empty(),
        "occurrence with lower confidence than threshold leaves without confirmation"
    );
    assert!(
        run.pool.confirmations.is_empty(),
        "server confirmation was never invoked for keyframe under confidence threshold"
    );
}

#[test]
fn low_contrast_observations_do_not_start_occurrences() {
    let run = Run::new(
        vec![writing(17..=85, 100, 100)],
        100,
        ShotChanges::default(),
    );
    assert!(
        run.document.occurrences.is_empty(),
        "flat surface without text contrast is discarded before starting an occurrence"
    );
}

#[test]
fn two_sample_transient_noise_does_not_probe_bisection() {
    let run = Run::with(
        vec![writing(17..=41, 20, 240)],
        80,
        ShotChanges::default(),
        Setup {
            min_bisection_samples: 3,
            ..Setup::default()
        },
    );
    assert!(
        run.document.occurrences.is_empty(),
        "transient 2-sample text does not qualify when min_bisection_samples is 3"
    );
    assert_eq!(
        run.stats.frames_probed, 0,
        "zero bisection probe frames are executed for unqualified transient noise"
    );
}

#[test]
fn three_sample_text_dispatches_entry_and_exit_bisection() {
    let run = Run::with(
        vec![writing(17..=55, 20, 240)],
        80,
        ShotChanges::default(),
        Setup {
            min_bisection_samples: 3,
            ..Setup::default()
        },
    );
    assert_eq!(
        run.document.occurrences.len(),
        1,
        "text persisting across 3 samples qualifies for persistence"
    );
    assert!(
        run.stats.frames_probed > 0,
        "entry and exit bisections are dispatched for persistent text"
    );
}

#[test]
fn cut_adjacent_two_sample_text_is_bisected() {
    let cuts = ShotChanges {
        cuts: vec![ShotCut {
            time_s: 15.0 / 24.0,
            score: 50.0,
        }],
    };
    let run = Run::with(
        vec![writing(15..=30, 20, 240)],
        80,
        cuts,
        Setup {
            min_bisection_samples: 3,
            ..Setup::default()
        },
    );
    assert_eq!(
        run.document.occurrences.len(),
        1,
        "cut-adjacent 2-sample text qualifies for bisection via touches_cut"
    );
    assert!(
        run.stats.frames_probed > 0,
        "bisections are dispatched for cut-adjacent text"
    );
}

#[test]
fn noise_that_never_qualifies_lets_go_of_its_keyframe_candidates_when_it_ends() {
    let setup = Setup {
        min_bisection_samples: 3,
        ..Setup::default()
    };
    let once = Run::with(
        vec![writing(17..=41, 20, 240)],
        80,
        ShotChanges::default(),
        setup,
    );
    // Five separate two-sample flickers, each gone a sample before the next appears.
    let flickers = (0..5u64)
        .map(|n| writing(17 + 48 * n..=41 + 48 * n, 20, 240))
        .collect();
    let five = Run::with(flickers, 260, ShotChanges::default(), setup);
    assert!(once.document.occurrences.is_empty());
    assert!(five.document.occurrences.is_empty());
    assert!(once.stats.keyframes_held_peak_bytes > 0);
    assert_eq!(
        five.stats.keyframes_held_peak_bytes, once.stats.keyframes_held_peak_bytes,
        "each flicker's samples go when it ends, so they never pile up"
    );
}

/// Writing first seen at one of the last samples of a screening group and qualifying for
/// bisection in the next group, with three samples required.
fn qualifying_in_the_next_group(first: u64) -> Run {
    // Blank samples repeat the first, so the first group closes with eight samples, 0 to 84.
    Run::with(
        vec![writing(first..=200, 20, 240)],
        240,
        ShotChanges::default(),
        Setup {
            min_bisection_samples: 3,
            ..Setup::default()
        },
    )
}

#[test]
fn writing_first_seen_at_a_groups_last_sample_keeps_its_exact_entry() {
    let run = qualifying_in_the_next_group(80);
    let [item] = run.document.occurrences.as_slice() else {
        panic!("one occurrence: {:?}", run.document.occurrences.len());
    };
    assert_eq!(item.start_s, run.time(80), "bisected in the next group");
    assert_eq!(item.end_s, run.time(201));
    assert_complete(&run, item);
}

#[test]
fn writing_first_seen_at_a_groups_second_to_last_sample_keeps_its_exact_entry() {
    let run = qualifying_in_the_next_group(68);
    let [item] = run.document.occurrences.as_slice() else {
        panic!("one occurrence: {:?}", run.document.occurrences.len());
    };
    assert_eq!(item.start_s, run.time(68), "bisected in the next group");
    assert_eq!(item.end_s, run.time(201));
    assert_complete(&run, item);
}

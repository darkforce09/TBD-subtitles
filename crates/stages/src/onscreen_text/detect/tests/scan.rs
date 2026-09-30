use super::*;
use image::Rgb;
use inference::ocr::OcrError;
use job_model::onscreen::{TextKeyframe, TextOccurrence};
use job_model::outputs::ShotCut;
use std::ops::RangeInclusive;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

const PROXY: (u32, u32) = (64, 36);
const SOURCE: (u32, u32) = (128, 72);
const BACKGROUND: [u8; 3] = [90, 110, 130];
/// The full-resolution detector's quads sit this far from the scaled screening quads.
const EXACT_OFFSET: f64 = 0.5;

struct Temporary(PathBuf);

impl Temporary {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "tbd-detect-scan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Scripted writing: a proxy rectangle, the frames that show it, and its ink on paper. The
/// pattern starts with ink at its top-left pixel, which the scripted detector looks for.
#[derive(Clone)]
struct Writing {
    rect: (u32, u32, u32, u32),
    frames: RangeInclusive<u64>,
    ink: u8,
    paper: u8,
}

fn rectangle(left: f64, top: f64, width: f64, height: f64) -> Quad {
    Quad([
        Point { x: left, y: top },
        Point {
            x: left + width,
            y: top,
        },
        Point {
            x: left + width,
            y: top + height,
        },
        Point {
            x: left,
            y: top + height,
        },
    ])
}

fn region(writing: &Writing, scale: u32, offset: f64) -> Quad {
    let (left, top, width, height) = writing.rect;
    rectangle(
        f64::from(left * scale) + offset,
        f64::from(top * scale) + offset,
        f64::from(width * scale),
        f64::from(height * scale),
    )
}

fn render(writings: &[Writing], index: u64, scale: u32) -> RgbImage {
    let mut image = RgbImage::from_pixel(PROXY.0 * scale, PROXY.1 * scale, Rgb(BACKGROUND));
    for writing in writings.iter().filter(|w| w.frames.contains(&index)) {
        let (left, top, width, height) = writing.rect;
        for y in 0..height * scale {
            for x in 0..width * scale {
                let inked = ((x / scale) / 3 + (y / scale) / 2).is_multiple_of(2);
                let value = if inked { writing.ink } else { writing.paper };
                image.put_pixel(left * scale + x, top * scale + y, Rgb([value; 3]));
            }
        }
    }
    image
}

struct Source {
    writings: Vec<Writing>,
    timeline: Vec<(f64, f64)>,
    next: u64,
    finished: bool,
    stills: Vec<Vec<u64>>,
}

impl FrameSource for Source {
    fn proxy_size(&self) -> (u32, u32) {
        PROXY
    }

    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn next_proxy(&mut self) -> TextResult<Option<ProxyFrame>> {
        let Some(&(time_s, end_s)) = self.timeline.get(self.next as usize) else {
            return Ok(None);
        };
        let index = self.next;
        self.next += 1;
        Ok(Some(ProxyFrame {
            index,
            time_s,
            end_s,
            rgb: render(&self.writings, index, 1),
        }))
    }

    fn finish_proxies(&mut self) -> TextResult<()> {
        self.finished = true;
        Ok(())
    }

    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>> {
        assert!(self.finished, "stills follow the finished proxy stream");
        assert!(indices.len() <= 4, "stills come four at a time");
        self.stills.push(indices.to_vec());
        Ok(indices
            .iter()
            .map(|&index| render(&self.writings, index, SOURCE.0 / PROXY.0))
            .collect())
    }
}

struct Detector {
    writings: Vec<Writing>,
    screened: usize,
    detected: usize,
}

impl Detector {
    fn found(&self, image: &RgbImage, offset: f64) -> Vec<(Quad, f64)> {
        let scale = image.width() / PROXY.0;
        self.writings
            .iter()
            .filter(|writing| {
                let (left, top, _, _) = writing.rect;
                image.get_pixel(left * scale, top * scale).0 == [writing.ink; 3]
            })
            .map(|writing| (region(writing, scale, offset), 0.9))
            .collect()
    }
}

impl TextDetection for Detector {
    fn screen_batch(&mut self, images: &[RgbImage]) -> Result<Vec<Vec<(Quad, f64)>>, OcrError> {
        assert!(images.iter().all(|image| image.dimensions() == PROXY));
        self.screened += images.len();
        Ok(images.iter().map(|image| self.found(image, 0.0)).collect())
    }

    fn detect(&mut self, image: &RgbImage) -> Result<Vec<(Quad, f64)>, OcrError> {
        assert_eq!(image.dimensions(), SOURCE);
        self.detected += 1;
        Ok(self.found(image, EXACT_OFFSET))
    }
}

struct Run {
    document: TextDocument,
    timeline: Vec<(f64, f64)>,
    source: Source,
    detector: Detector,
    progress: Vec<(usize, usize)>,
    root: Temporary,
}

impl Run {
    fn new(writings: Vec<Writing>, count: u64, cuts: ShotChanges) -> Self {
        let timeline: Vec<(f64, f64)> = (0..count)
            .map(|i| (i as f64 / 24.0, (i + 1) as f64 / 24.0))
            .collect();
        let mut source = Source {
            writings: writings.clone(),
            timeline: timeline.clone(),
            next: 0,
            finished: false,
            stills: Vec::new(),
        };
        let mut detector = Detector {
            writings,
            screened: 0,
            detected: 0,
        };
        let stream = VideoStream {
            index: 0,
            codec: "h264".into(),
            width: SOURCE.0,
            height: SOURCE.1,
            frame_rate_num: 24,
            frame_rate_den: 1,
            start_time_s: 0.0,
            ..Default::default()
        };
        let root = Temporary::new();
        let calls = Mutex::new(Vec::new());
        let progress = |done: usize, total: usize| calls.lock().unwrap().push((done, total));
        let document = scan(
            &mut source,
            &stream,
            &cuts,
            &root.0,
            &mut detector,
            &progress,
        )
        .unwrap();
        Self {
            document,
            timeline,
            source,
            detector,
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
}

fn writing(frames: RangeInclusive<u64>, ink: u8, paper: u8) -> Writing {
    Writing {
        rect: (8, 6, 40, 10),
        frames,
        ink,
        paper,
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

#[test]
fn a_region_gets_its_exact_first_and_last_frame_one_crop_and_one_keyframe() {
    let run = Run::new(vec![writing(17..=41, 20, 240)], 80, ShotChanges::default());
    let document = &run.document;
    assert_eq!(document.sample_step, 12);
    assert_eq!(document.proxy_width, PROXY.0);
    assert_eq!((document.width, document.height), SOURCE);
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
    let screening = region(&run.detector.writings[0], 2, 0.0);
    assert_eq!(item.frames[0].quad, screening, "quads are in source pixels");
    assert_eq!(
        item.frames[1].quad,
        region(&run.detector.writings[0], 2, EXACT_OFFSET)
    );
    assert_eq!(item.frames[2].quad, screening);
    assert!(item.warnings.is_empty());
    assert_eq!(run.files("visual/crops"), ["text-000001.png"]);
    assert_eq!(run.files("visual/keyframes"), ["frame-00000024.png"]);
    assert_eq!(run.source.stills, [[24]]);
    assert_eq!(run.detector.detected, 1);
    assert!(
        run.detector.screened <= 3 + 2 * 4,
        "repeated samples reuse screens: {} images",
        run.detector.screened
    );
    assert!(run.progress[..80].iter().all(|&(_, total)| total == 0));
    assert_eq!(run.progress.last(), Some(&(81, 81)));
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
    let timeline_60 = 60.0 / 24.0;
    let cuts = ShotChanges {
        cuts: vec![ShotCut {
            time_s: timeline_60,
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
fn the_scanned_document_deserialises_with_its_keyframes_and_proxy_width() {
    let run = Run::new(vec![writing(17..=41, 20, 240)], 80, ShotChanges::default());
    let json = serde_json::to_string(&run.document).unwrap();
    assert!(json.contains("\"proxy_width\":64"));
    assert!(json.contains("\"sample_step\":12"));
    assert!(json.contains("\"keyframe\":{"));
    let back: TextDocument = serde_json::from_str(&json).unwrap();
    assert_eq!((back.proxy_width, back.sample_step), (64, 12));
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
    assert!((item.start_s - original.start_s).abs() < 1e-9);
    assert!((item.end_s - original.end_s).abs() < 1e-9);
}

#[test]
fn a_video_without_writing_screens_only_its_samples() {
    let run = Run::new(Vec::new(), 50, ShotChanges::default());
    assert!(run.document.occurrences.is_empty());
    assert_eq!(run.document.decoded_frames, 50);
    assert_eq!(
        run.detector.screened, 1,
        "every later sample repeats the first"
    );
    assert_eq!(run.detector.detected, 0);
    assert!(run.source.stills.is_empty());
    assert_eq!(run.progress.len(), 50);
}

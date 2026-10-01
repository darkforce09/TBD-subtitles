use job_model::outputs::ShotChanges;

use super::*;
use crate::onscreen_text::detect::fixtures::{Answer, Pool, Source, Temporary, stream, writing};
use crate::onscreen_text::detect::{scan, scan_measured};

#[test]
fn a_measured_scan_returns_the_plain_scan_s_document_and_counts_its_frames() {
    let writings = vec![writing(17..=41, 20, 240)];
    let cuts = ShotChanges::default();
    let folder = Temporary::new("timing");
    let mut plain_pool = Pool::new(writings.clone(), 2, Answer::Oldest);
    let plain = scan(
        &mut Source::new(writings.clone(), 50),
        &stream(),
        &cuts,
        &folder.0,
        &mut plain_pool,
        &|_, _| {},
    )
    .unwrap();
    let mut pool = Pool::new(writings.clone(), 2, Answer::Oldest);
    let (document, stats) = scan_measured(
        &mut Source::new(writings, 50),
        &stream(),
        &cuts,
        &folder.0,
        &mut pool,
        &|_, _| {},
    )
    .unwrap();
    assert_eq!(document, plain);
    assert_eq!(stats.frames_decoded, 50);
    assert_eq!(stats.frames_screened, pool.screened as u64);
    assert_eq!(pool.screened, plain_pool.screened);
    assert_eq!(stats.keyframes_from_ram, 1);
}

#[test]
fn the_notes_name_every_phase_count_and_session_note() {
    let stats = ScanStats {
        decode_wait: Duration::from_millis(1_260),
        frames_decoded: 80,
        keyframes_held_peak_bytes: 3 << 20,
        warmup_s: 2.04,
        notes: BTreeMap::from([("engine".to_string(), "cuda".to_string())]),
        ..ScanStats::default()
    };
    let notes: BTreeMap<String, String> = stats.notes().into_iter().collect();
    for key in [
        "decode_wait_s",
        "convert_s",
        "screen_s",
        "probe_s",
        "signature_s",
        "confirm_s",
        "stills_ram_s",
        "stills_ffmpeg_s",
        "warmup_s",
        "engine_build_s",
        "frames_decoded",
        "frames_screened",
        "frames_probed",
        "keyframes_ram",
        "keyframes_ffmpeg",
        "keyframes_held_peak_mib",
        "detector_engine",
    ] {
        assert!(notes.contains_key(key), "{key}");
    }
    assert_eq!(notes["decode_wait_s"], "1.3");
    assert_eq!(notes["warmup_s"], "2.0");
    assert_eq!(notes["frames_decoded"], "80");
    assert_eq!(notes["keyframes_held_peak_mib"], "3");
    assert_eq!(notes["detector_engine"], "cuda");
}

#[test]
fn a_timed_phase_adds_the_time_its_work_took() {
    let mut phase = Duration::from_secs(1);
    let value = timed(&mut phase, || {
        std::thread::sleep(Duration::from_millis(5));
        7
    });
    assert_eq!(value, 7);
    assert!(phase >= Duration::from_millis(1_005));
}

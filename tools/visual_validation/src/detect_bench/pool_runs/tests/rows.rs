use super::*;
use inference::ocr::pool::ScreenShape;
use job_model::onscreen::{Point, Quad};

const SHAPE: ScreenShape = ScreenShape {
    batch: 4,
    pool_mib: 2048,
};

fn notes(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

fn measured() -> Measured {
    let quad = Quad([Point { x: 0.0, y: 0.0 }; 4]);
    Measured {
        frames: 240,
        usage: Usage {
            wall_s: 1.6,
            cpu_cores: Some(3.2),
            gpu_pct: Some(97.4),
            decoder_pct: None,
        },
        vram: Some(VramPeaks {
            process_mib: 4410,
            device_delta_mib: 4562,
            samples: 30,
        }),
        warmup_s: 3.3,
        engine_build_s: 0.0,
        notes: notes(&[
            (
                "screen session options (thread 1)",
                "CUDA graph and NHWC on",
            ),
            (
                "screen session options (thread 2)",
                "CUDA graph and NHWC on",
            ),
        ]),
        regions: vec![vec![(quad, 0.9), (quad, 0.8)], vec![], vec![(quad, 0.7)]],
    }
}

#[test]
fn a_measured_row_fills_every_column() {
    let run = PoolRun::cuda(SHAPE, 2);
    let cells = row(&run, Phase::Screen, &Ok(measured()), Some("identical"));
    assert_eq!(cells.len(), COLUMNS.len());
    assert_eq!(
        cells,
        [
            "CUDA",
            "fast",
            "2",
            "4",
            "2048",
            "240",
            "150.0",
            "6.67",
            "97",
            "4410",
            "4562",
            "3.3",
            "0.0",
            "—",
            "accepted ×2",
            "3",
            "identical",
            ""
        ]
    );
}

#[test]
fn a_failed_row_keeps_its_configuration_and_ends_with_the_error() {
    let run = PoolRun::tensorrt(SHAPE, 2, false);
    let outcome = Err("TensorRT could not build the engine".to_string());
    let cells = row(&run, Phase::Screen, &outcome, None);
    assert_eq!(cells.len(), COLUMNS.len());
    assert_eq!(&cells[..5], ["TensorRT FP32", "fast", "2", "4", "2048"]);
    assert!(cells[5..17].iter().all(String::is_empty));
    assert_eq!(cells[17], "error: TensorRT could not build the engine");
}

#[test]
fn a_confirm_row_shows_its_sessions_batch_one_and_the_confirm_pool() {
    let run = PoolRun::cuda(SHAPE, 2);
    let mut confirmed = measured();
    confirmed.vram = None;
    confirmed.usage.gpu_pct = None;
    let cells = row(&run, Phase::Confirm, &Ok(confirmed), None);
    assert_eq!(cells[2], inference::ocr::pool::CONFIRM_SESSIONS.to_string());
    assert_eq!(cells[3], "1");
    assert_eq!(cells[4], inference::ocr::pool::CONFIRM_POOL_MIB.to_string());
    assert_eq!(cells[8], "n/a");
    assert_eq!(cells[9], "n/a");
    assert_eq!(cells[14], "n/a");
    assert_eq!(cells[16], "—");
}

#[test]
fn a_refused_graph_names_the_count_and_the_first_reason() {
    let notes = notes(&[
        (
            "screen session options (thread 1)",
            "CUDA graph and NHWC on",
        ),
        (
            "screen session options (thread 2)",
            "CUDA graph and NHWC off; the session refused them: graph capture failed",
        ),
        (
            "confirm session options (thread 1)",
            "CUDA graph and NHWC on",
        ),
    ]);
    assert_eq!(
        tuning(&notes, Phase::Screen),
        "refused ×1 of 2: graph capture failed"
    );
    assert_eq!(tuning(&notes, Phase::Confirm), "accepted ×1");
}

#[test]
fn engine_cache_notes_say_built_or_reused() {
    let notes = notes(&[
        ("screen engine cache (thread 1)", "built into /tmp/trt/ab12"),
        (
            "screen engine cache (thread 2)",
            "reused from /tmp/trt/ab12",
        ),
        (
            "confirm engine cache (thread 1)",
            "reused from /tmp/trt/cd34",
        ),
    ]);
    assert_eq!(engine_cache(&notes, Phase::Screen), "built ×1, reused ×1");
    assert_eq!(engine_cache(&notes, Phase::Confirm), "reused ×1");
    assert_eq!(engine_cache(&BTreeMap::new(), Phase::Screen), "—");
}

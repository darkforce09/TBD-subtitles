use super::*;

fn shape(batch: usize, pool_mib: usize) -> ScreenShape {
    ScreenShape { batch, pool_mib }
}

#[test]
fn the_default_sweep_is_three_batches_by_four_pools_batch_first() {
    let shapes = grid(&[2, 4, 8], &[1536, 2048, 2560, 3072]);
    assert_eq!(shapes.len(), 12);
    assert_eq!(
        &shapes[..4],
        &[
            shape(2, 1536),
            shape(2, 2048),
            shape(2, 2560),
            shape(2, 3072)
        ]
    );
    assert_eq!(shapes[4], shape(4, 1536));
    assert_eq!(shapes[11], shape(8, 3072));
}

#[test]
fn zero_and_repeated_values_leave_the_grid() {
    let shapes = grid(&[0, 4, 4], &[2048, 0, 2048, 1024]);
    assert_eq!(shapes, vec![shape(4, 2048), shape(4, 1024)]);
    assert!(grid(&[], &[2048]).is_empty());
    assert!(grid(&[4], &[]).is_empty());
}

#[test]
fn the_fastest_row_wins_and_the_first_of_equals_stays() {
    let measured = [
        (shape(2, 1536), 140.0),
        (shape(4, 2048), 181.5),
        (shape(8, 3072), 181.5),
        (shape(8, 2048), f64::NAN),
    ];
    assert_eq!(fastest(&measured), Some(shape(4, 2048)));
    assert_eq!(fastest(&[]), None);
}

#[test]
fn a_given_shape_beats_the_sweep_and_the_default_fills_in() {
    let measured = [(shape(8, 3072), 200.0)];
    assert_eq!(chosen(Some(shape(2, 1536)), &measured), shape(2, 1536));
    assert_eq!(chosen(None, &measured), shape(8, 3072));
    assert_eq!(chosen(None, &[]), ScreenShape::INITIAL);
}

#[test]
fn labels_name_the_engine_precision_and_search() {
    let cuda = PoolRun::cuda(shape(4, 2048), 2);
    assert_eq!(cuda.engine_label(), "CUDA");
    assert_eq!(cuda.search_label(), "fast");
    assert_eq!(
        PoolRun::tensorrt(shape(4, 2048), 2, true).engine_label(),
        "TensorRT FP16"
    );
    let fp32 = PoolRun::tensorrt(shape(4, 2048), 1, false);
    assert_eq!(fp32.engine_label(), "TensorRT FP32");
    assert_eq!(fp32.sessions, 1);
    let deterministic = PoolRun {
        search: SearchMode::Deterministic,
        ..cuda
    };
    assert_eq!(deterministic.search_label(), "deterministic");
}

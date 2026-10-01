use super::super::Role;
use super::*;
use crate::ocr::detector_pool::batch::InputShape;

fn spec(engine: DetectorEngine, search: SearchMode) -> SessionSpec {
    SessionSpec {
        role: Role::Screen,
        model: PathBuf::from("det_mobile.onnx"),
        input: InputShape::for_frames(4, 1920, 1080),
        pool_mib: 2_304,
        engine,
        search,
        tensorrt: None,
    }
}

fn plan() -> TensorRtPlan {
    TensorRtPlan {
        cache_dir: PathBuf::from("/data/tensorrt/0123abcd"),
        profile: "x:4x3x1088x1920".to_owned(),
        fp16: true,
        workspace_mib: 768,
    }
}

/// The options a provider was given, as its debug text shows them.
fn options(provider: &impl std::fmt::Debug) -> String {
    format!("{provider:?}")
}

fn has(options: &str, key: &str, value: &str) -> bool {
    options.contains(&format!("\"{key}\": \"{value}\""))
}

#[test]
fn tensorrt_registers_before_cuda_and_both_fail_loudly() {
    let spec = spec(DetectorEngine::TensorRt, SearchMode::Fast);
    let list = providers(&spec, CudaTuning::FULL, Some(&plan()));
    let described: Vec<String> = list.iter().map(|p| format!("{p:?}")).collect();
    assert_eq!(described.len(), 2);
    assert!(
        described[0].starts_with("TensorrtExecutionProvider"),
        "{}",
        described[0]
    );
    assert!(
        described[1].starts_with("CUDAExecutionProvider"),
        "{}",
        described[1]
    );
    assert!(
        described
            .iter()
            .all(|d| d.contains("error_on_failure: true"))
    );
    assert!(list[0].downcast_ref::<ep::TensorRT>().is_some());
    assert!(list[1].downcast_ref::<ep::CUDA>().is_some());
}

#[test]
fn the_cuda_engine_registers_cuda_alone() {
    let spec = spec(DetectorEngine::Cuda, SearchMode::Fast);
    let list = providers(&spec, CudaTuning::FULL, None);
    assert_eq!(list.len(), 1);
    assert!(format!("{:?}", list[0]).starts_with("CUDAExecutionProvider"));
}

#[test]
fn the_fast_search_is_exhaustive_with_the_largest_workspace() {
    let cuda = options(&cuda(
        &spec(DetectorEngine::Cuda, SearchMode::Fast),
        CudaTuning::FULL,
    ));
    assert!(has(&cuda, "cudnn_conv_algo_search", "EXHAUSTIVE"), "{cuda}");
    assert!(has(&cuda, "cudnn_conv_use_max_workspace", "1"));
    assert!(has(&cuda, "gpu_mem_limit", &(2_304usize << 20).to_string()));
    assert!(has(&cuda, "arena_extend_strategy", "kSameAsRequested"));
    assert!(has(&cuda, "use_tf32", "1"));
    assert!(has(&cuda, "prefer_nhwc", "1"));
    assert!(has(&cuda, "enable_cuda_graph", "1"));
    assert!(!deterministic_compute(SearchMode::Fast));
}

#[test]
fn the_deterministic_search_is_heuristic_with_deterministic_kernels() {
    let cuda = options(&cuda(
        &spec(DetectorEngine::Cuda, SearchMode::Deterministic),
        CudaTuning::FULL,
    ));
    assert!(has(&cuda, "cudnn_conv_algo_search", "HEURISTIC"), "{cuda}");
    assert!(has(&cuda, "cudnn_conv_use_max_workspace", "0"));
    assert!(deterministic_compute(SearchMode::Deterministic));
}

#[test]
fn the_plain_reopen_drops_the_graph_and_nhwc() {
    let cuda = options(&cuda(
        &spec(DetectorEngine::Cuda, SearchMode::Fast),
        CudaTuning::PLAIN,
    ));
    assert!(has(&cuda, "prefer_nhwc", "0"));
    assert!(has(&cuda, "enable_cuda_graph", "0"));
    assert!(has(&cuda, "use_tf32", "1"));
}

#[test]
fn under_tensorrt_the_graph_is_tensorrt_s_own() {
    let spec = spec(DetectorEngine::TensorRt, SearchMode::Fast);
    let cuda = options(&cuda(&spec, CudaTuning::FULL));
    assert!(has(&cuda, "enable_cuda_graph", "0"), "{cuda}");
    let trt = options(&tensorrt(&plan(), CudaTuning::FULL));
    assert!(has(&trt, "trt_cuda_graph_enable", "1"), "{trt}");
    let plain = options(&tensorrt(&plan(), CudaTuning::PLAIN));
    assert!(has(&plain, "trt_cuda_graph_enable", "0"));
}

#[test]
fn tensorrt_builds_one_cached_fp16_shape_within_its_workspace() {
    let trt = options(&tensorrt(&plan(), CudaTuning::FULL));
    assert!(has(&trt, "trt_fp16_enable", "1"), "{trt}");
    assert!(has(&trt, "trt_engine_cache_enable", "1"));
    assert!(has(
        &trt,
        "trt_engine_cache_path",
        "/data/tensorrt/0123abcd"
    ));
    assert!(has(&trt, "trt_timing_cache_enable", "1"));
    assert!(has(
        &trt,
        "trt_timing_cache_path",
        "/data/tensorrt/0123abcd"
    ));
    for bound in ["min", "opt", "max"] {
        let key = format!("trt_profile_{bound}_shapes");
        assert!(has(&trt, &key, "x:4x3x1088x1920"), "{key}");
    }
    assert!(has(
        &trt,
        "trt_max_workspace_size",
        &(768usize << 20).to_string()
    ));
    let fp32 = TensorRtPlan {
        fp16: false,
        ..plan()
    };
    assert!(has(
        &options(&tensorrt(&fp32, CudaTuning::FULL)),
        "trt_fp16_enable",
        "0"
    ));
}

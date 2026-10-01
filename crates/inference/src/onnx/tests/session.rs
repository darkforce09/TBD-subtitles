//! Checks of the CUDA provider every session registers, read from its options without a GPU.

use super::{SESSION_MEMORY_LIMIT, cuda_provider};

fn provider_options() -> String {
    format!("{:?}", cuda_provider())
}

#[test]
fn the_session_memory_limit_is_four_and_a_half_gibibytes() {
    assert_eq!(SESSION_MEMORY_LIMIT, 4_831_838_208);
}

#[test]
fn the_cuda_provider_caps_the_arena_at_the_session_memory_limit() {
    let options = provider_options();
    assert!(options.contains("\"gpu_mem_limit\""), "{options}");
    assert!(
        options.contains(&format!("\"{SESSION_MEMORY_LIMIT}\"")),
        "{options}"
    );
}

#[test]
fn the_cuda_provider_grows_the_arena_by_what_is_requested() {
    let options = provider_options();
    assert!(options.contains("\"arena_extend_strategy\""), "{options}");
    assert!(options.contains("\"kSameAsRequested\""), "{options}");
}

#[test]
fn the_cuda_provider_keeps_the_default_convolution_search() {
    let options = provider_options();
    assert!(!options.contains("cudnn_conv_algo_search"), "{options}");
    assert!(
        !options.contains("cudnn_conv_use_max_workspace"),
        "{options}"
    );
}

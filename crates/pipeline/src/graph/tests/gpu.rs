use super::*;

#[test]
fn every_gpu_step_needs_memory_within_the_cap_and_no_other_step_does() {
    for step in StepName::ALL {
        match vram_need_mib(step) {
            Some(need) => {
                assert!(uses_gpu(step), "{step}");
                assert!(need > VRAM_HEADROOM_MIB, "{step}");
                assert!(need <= WORKER_VRAM_CAP_MIB, "{step}");
            }
            None => assert!(!uses_gpu(step), "{step}"),
        }
    }
}

#[test]
fn a_need_is_the_measured_peak_with_headroom_never_the_cap() {
    assert_eq!(vram_need_mib(StepName::Separation), Some(4_430));
    assert_eq!(vram_need_mib(StepName::AsrWhisper), Some(4_668));
    assert_eq!(
        vram_need_mib(StepName::TextDetect),
        Some(TEXT_DETECT_VRAM_MIB)
    );
    assert!(vram_need_mib(StepName::LocalizedVideo).unwrap() < 1_024);
}

#[test]
fn only_translation_locks_the_gpu_when_its_model_loads() {
    for step in StepName::ALL {
        assert_eq!(
            locks_gpu_lazily(step),
            step == StepName::TextTranslate,
            "{step}"
        );
    }
    assert!(uses_gpu(StepName::TextTranslate));
}

#[test]
fn the_visual_lane_gives_way_to_the_audio_steps() {
    assert_eq!(gpu_priority(StepName::TextDetect), GpuPriority::Visual);
    assert_eq!(gpu_priority(StepName::TextRead), GpuPriority::Visual);
    assert_eq!(gpu_priority(StepName::RedecodeParakeet), GpuPriority::Audio);
    assert_eq!(gpu_priority(StepName::TextInpaint), GpuPriority::Audio);
}

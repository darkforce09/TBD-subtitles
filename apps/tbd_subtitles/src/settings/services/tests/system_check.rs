use super::*;

fn info(free_mib: u64) -> DeviceInfo {
    DeviceInfo {
        name: "NVIDIA GeForce RTX 3070".into(),
        driver: "615.0".into(),
        total_mib: 8192,
        free_mib,
    }
}

#[test]
fn a_gpu_with_room_for_a_step_is_ok() {
    let check = gpu(Some(info(7000)));
    assert_eq!(check.state, CheckState::Ok);
    assert!(check.detail.contains("RTX 3070"), "{}", check.detail);
    assert!(check.detail.contains("615.0"), "{}", check.detail);
}

#[test]
fn too_little_free_memory_is_a_warning_naming_the_budget() {
    let check = gpu(Some(info(4000)));
    assert_eq!(check.state, CheckState::Warning);
    assert!(check.detail.contains("6656"), "{}", check.detail);
}

#[test]
fn no_driver_is_a_failure_never_a_pass() {
    assert_eq!(gpu(None).state, CheckState::Failed);
}

#[test]
fn a_version_check_shows_the_first_line_or_the_reason() {
    let ok = version_check(
        "FFmpeg",
        Ok("\nffmpeg version 8.1.2\nbuilt with gcc\n".into()),
    );
    assert_eq!(ok.state, CheckState::Ok);
    assert_eq!(ok.detail, "ffmpeg version 8.1.2");
    let failed = version_check("claude CLI", Err("claude not found on PATH".into()));
    assert_eq!(failed.state, CheckState::Failed);
    assert_eq!(failed.detail, "claude not found on PATH");
}

#[test]
fn with_source_prefixes_bundled_versus_path() {
    assert_eq!(
        with_source(true, "ffmpeg version 8.1"),
        "bundled: ffmpeg version 8.1"
    );
    assert_eq!(
        with_source(false, "ffmpeg version 8.1"),
        "on PATH: ffmpeg version 8.1"
    );
}

#[test]
fn clip_sound_needs_the_pulse_output_device() {
    let with = " DE alsa            ALSA audio output\n DE pulse           Pulse audio output\n";
    assert_eq!(pulse_output(Ok(with.into())).state, CheckState::Ok);
    let without = " DE alsa            ALSA audio output\n";
    assert_eq!(pulse_output(Ok(without.into())).state, CheckState::Warning);
    assert_eq!(pulse_output(Err("gone".into())).state, CheckState::Warning);
}

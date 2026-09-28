use super::*;

#[test]
fn the_playhead_follows_the_time_since_the_sound_began_and_stops_at_the_end() {
    let clip = Clip::around(10.0, 12.0, PAD_S);
    assert_eq!(clip.start_s, 9.25);
    let began = Instant::now();
    assert_eq!(
        position_at(clip, None, began),
        9.25,
        "before the sound starts"
    );
    let later = began + Duration::from_millis(1500);
    assert!((position_at(clip, Some(began), later) - 10.75).abs() < 1e-9);
    let past = began + Duration::from_secs(60);
    assert_eq!(
        position_at(clip, Some(began), past),
        9.25 + 3.5,
        "capped at the end"
    );
    assert_eq!(
        position_at(clip, Some(later), began),
        9.25,
        "never before the start"
    );
}

#[test]
fn every_frame_has_a_serial_of_its_own() {
    let a = frame_of((2, 2), vec![0; 16]);
    let b = frame_of((2, 2), vec![0; 16]);
    assert_ne!(a.serial, b.serial);
}

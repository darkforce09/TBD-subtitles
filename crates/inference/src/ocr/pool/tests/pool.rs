use super::*;

#[test]
fn heights_pad_to_the_next_multiple_of_32() {
    assert_eq!(PaddedFrame::padded(1080), 1088);
    assert_eq!(PaddedFrame::padded(1088), 1088);
    assert_eq!(PaddedFrame::padded(720), 736);
    assert_eq!(PaddedFrame::padded(1), 32);
}

#[test]
fn probes_come_before_screening_batches() {
    let mut waiting = [Priority::Screen, Priority::Probe, Priority::Screen];
    waiting.sort();
    assert_eq!(waiting[0], Priority::Probe);
}

#[test]
fn the_initial_shape_keeps_eight_frames_in_flight_on_two_sessions() {
    assert_eq!(ScreenShape::INITIAL.batch * SCREEN_SESSIONS, 8);
}

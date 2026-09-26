use super::*;

#[test]
fn a_clip_is_padded_but_never_before_the_video() {
    assert_eq!(
        Clip::around(10.0, 12.0, 0.5),
        Clip {
            start_s: 9.5,
            duration_s: 3.0
        }
    );
    assert_eq!(Clip::around(0.2, 0.3, 0.5).start_s, 0.0);
    assert!(Clip::around(5.0, 4.0, 0.0).duration_s >= 0.1);
}

#[test]
fn the_track_sound_goes_to_the_pulse_device() {
    let args = track_sound(Path::new("/v/a.mp4"), 1, Clip::around(10.0, 12.0, 0.5));
    let joined = args.join(" ");
    assert!(
        joined.contains("-ss 9.500 -t 3.000 -i /v/a.mp4 -map 0:a:1"),
        "{joined}"
    );
    assert!(
        joined.ends_with("-vn -af apad=pad_dur=1 -buffer_duration 200 -f pulse TBD Subtitles clip"),
        "{joined}"
    );
}

#[test]
fn a_stem_is_read_as_raw_16_khz_mono() {
    let joined = stem_sound(Path::new("/w/vocals_16k.f32"), Clip::around(1.0, 2.0, 0.0)).join(" ");
    assert!(joined.contains("-f f32le -ar 16000 -ac 1 -ss 1.000 -t 1.000 -i /w/vocals_16k.f32"));
    assert!(
        joined.ends_with(
            "/w/vocals_16k.f32 -af apad=pad_dur=1 -buffer_duration 200 -f pulse TBD Subtitles clip"
        ),
        "{joined}"
    );
}

#[test]
fn frames_keep_the_shape_on_even_sizes() {
    assert_eq!(frame_size(1920, 1080, 360), (640, 360));
    assert_eq!(frame_size(1440, 1080, 360), (480, 360));
    assert_eq!(frame_size(0, 0, 361), (640, 360));
    let joined = frames(
        Path::new("/v/a.mp4"),
        Clip::around(1.0, 2.0, 0.0),
        (640, 360),
        12,
    )
    .join(" ");
    assert!(joined.contains("-vf scale=640:360,fps=12 -f rawvideo -pix_fmt rgba pipe:1"));
}

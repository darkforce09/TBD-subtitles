use super::*;

const FRAME: f64 = 1.0 / 24.0;

fn constant(frames: usize, offset: f64) -> Vec<(f64, f64)> {
    (0..frames)
        .map(|i| (offset + i as f64 * FRAME, offset + (i + 1) as f64 * FRAME))
        .collect()
}

#[test]
fn timelines_compare_relative_to_their_first_frames() {
    let source = constant(48, 0.0);
    assert_eq!(
        compare_timelines(&source, &constant(48, 0.5), FRAME),
        Ok(())
    );
    // Millisecond rounding is within half a frame.
    let mut rounded = constant(48, 0.0);
    rounded[20].0 += 0.001;
    assert_eq!(compare_timelines(&source, &rounded, FRAME), Ok(()));
    assert_eq!(compare_timelines(&[], &[], FRAME), Ok(()));
}

#[test]
fn a_missing_shifted_or_stretched_frame_fails_the_timeline() {
    let source = constant(48, 0.0);
    let short = compare_timelines(&source, &constant(47, 0.0), FRAME).unwrap_err();
    assert_eq!(short.0, "the joined video has 47 frames, the source 48");
    let mut shifted = constant(48, 0.0);
    for frame in &mut shifted[24..] {
        frame.0 += FRAME;
        frame.1 += FRAME;
    }
    let reason = compare_timelines(&source, &shifted, FRAME).unwrap_err().0;
    assert!(
        reason.starts_with("frame 24 of the joined video"),
        "{reason}"
    );
    let mut stretched = constant(48, 0.0);
    stretched[47].1 += FRAME;
    assert!(compare_timelines(&source, &stretched, FRAME).is_err());
}

fn starts(video: f64, audio: Option<f64>) -> StreamStarts {
    StreamStarts {
        video_s: Some(video),
        audio_s: audio,
    }
}

#[test]
fn the_video_must_start_as_far_from_the_audio_as_in_the_source() {
    let source = starts(0.0, Some(-0.023));
    assert_eq!(
        compare_starts(source, starts(0.023, Some(0.0)), FRAME),
        Ok(())
    );
    assert_eq!(
        compare_starts(source, starts(0.033, Some(0.0)), FRAME),
        Ok(())
    );
    let lost = compare_starts(source, starts(0.0, Some(0.0)), FRAME).unwrap_err();
    assert_eq!(
        lost.0,
        "the joined video starts 0.000 s after its audio, the source's 0.023 s"
    );
    let silent = compare_starts(source, starts(0.0, None), FRAME).unwrap_err();
    assert!(silent.0.contains("unknown"), "{}", silent.0);
    assert_eq!(
        compare_starts(starts(0.5, None), starts(0.0, None), FRAME),
        Ok(())
    );
}

fn pieces() -> Vec<Piece> {
    vec![
        Piece::Copy { first: 0, last: 95 },
        Piece::Encode {
            first: 96,
            last: 119,
        },
        Piece::Copy {
            first: 120,
            last: 239,
        },
    ]
}

#[test]
fn decode_starts_are_the_last_keyframes_well_before_each_join() {
    let keyframes = [0, 24, 48, 72, 96, 108, 120, 144];
    assert_eq!(start_candidates(&pieces(), &keyframes, 48), [0, 24, 48, 72]);
    assert_eq!(start_candidates(&pieces()[..1], &keyframes, 48), [0u64; 0]);
}

#[test]
fn decode_windows_run_from_a_decode_start_past_each_join() {
    // Joins at 96 and 120; lead 48 frames and lag 24.
    assert_eq!(
        decode_windows(&pieces(), 240, 48, 24, &[0, 48, 96, 120]),
        [(48, 144)]
    );
    // Far-apart joins get their own windows; the last is cut at the end of the video.
    let far = [
        Piece::Copy {
            first: 0,
            last: 499,
        },
        Piece::Encode {
            first: 500,
            last: 599,
        },
        Piece::Copy {
            first: 600,
            last: 619,
        },
    ];
    assert_eq!(
        decode_windows(&far, 620, 30, 24, &[0, 400, 500, 560, 600]),
        [(400, 524), (560, 620)]
    );
    // With no start before the lead, the decode starts at the first frame.
    assert_eq!(
        decode_windows(&pieces(), 240, 200, 24, &[0, 120]),
        [(0, 144)]
    );
}

#[test]
fn the_decode_check_seeks_to_its_keyframe_and_prints_only_errors() {
    assert_eq!(
        decode_args(Path::new("/v/out.mkv"), 1.010417, 3.5).join(" "),
        "-nostdin -hide_banner -v error -ss 1.010417 -i /v/out.mkv -t 3.500000 -map 0:v:0 -f null -"
    );
}

//! The segment encode's choices, and FFmpeg round trips of the whole render: a segment path with
//! one patch, and the sources and plans that fall back to the whole-video encode.

use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use image::{Rgba, RgbaImage};
use job_model::onscreen::{
    PixelRect, Plate, ReplaceStatus, ReplacedText, ReplacementDocument, SegmentSummary,
};
use media_io::Programs;
use media_io::video_frames::timeline;

use super::*;
use crate::localize::motion::Motion;
use crate::localize::whole::whole_summary;
use crate::localize::{RenderRequest, render, settle};

fn rendered(encoder: &'static str) -> Rendered {
    Rendered {
        frames: 10,
        encoder,
        phases: RenderPhases::default(),
        segments: SegmentSummary::default(),
    }
}

#[test]
fn a_finished_segment_encode_is_kept_without_the_whole_encode() {
    let ran = Cell::new(false);
    let spent = RenderPhases::default();
    let result = settle(Ok(rendered("libx264")), spent, |_, _| {
        ran.set(true);
        Ok(rendered("hevc_nvenc"))
    });
    assert_eq!(result.unwrap().encoder, "libx264");
    assert!(!ran.get());
}

#[test]
fn every_fallback_reason_runs_the_whole_encode_with_that_reason_and_the_time_spent() {
    let reasons = [
        "the video is hevc, not H.264",
        "the video has a variable frame rate",
        "invalid segment plan: the keyframes are not ascending frames of the video",
        "every frame lies in a re-encoded segment",
        "could not copy the unchanged pieces: ffmpeg exited with code 1",
        "the joined video decodes with errors around frame 480",
    ];
    for reason in reasons {
        let spent = RenderPhases {
            plan: Duration::from_millis(250),
            ..RenderPhases::default()
        };
        let attempt = Err(Attempt::Fallback(FallbackReason(reason.into())));
        let result = settle(attempt, spent, |reason, phases| {
            assert_eq!(phases, spent);
            Ok(Rendered {
                segments: whole_summary(10, reason),
                ..rendered("hevc_nvenc")
            })
        })
        .unwrap();
        assert_eq!(result.encoder, "hevc_nvenc");
        assert_eq!(
            result.segments,
            SegmentSummary {
                segments_reencoded: 1,
                frames_reencoded: 10,
                frames_copied: 0,
                fallback_reason: Some(reason.into()),
            }
        );
    }
}

#[test]
fn a_stopped_attempt_never_falls_back() {
    let ran = Cell::new(false);
    let result = settle(
        Err(Attempt::Failed("the localized video was cancelled".into())),
        RenderPhases::default(),
        |_, _| {
            ran.set(true);
            Ok(rendered("libx264"))
        },
    );
    assert_eq!(result.unwrap_err().0, "the localized video was cancelled");
    assert!(!ran.get());
}

#[test]
fn a_plan_that_copies_nothing_is_not_worth_joining() {
    let all_encoded = [
        Piece::Encode { first: 0, last: 47 },
        Piece::Encode {
            first: 48,
            last: 99,
        },
    ];
    assert_eq!(
        worth_joining(&all_encoded),
        Err(FallbackReason(
            "every frame lies in a re-encoded segment".into()
        ))
    );
    let mixed = [
        Piece::Copy { first: 0, last: 47 },
        Piece::Encode {
            first: 48,
            last: 99,
        },
    ];
    assert_eq!(worth_joining(&mixed), Ok(()));
    assert_eq!(worth_joining(&[Piece::Copy { first: 0, last: 99 }]), Ok(()));
}

#[test]
fn changed_runs_become_planner_spans_and_encoders_their_ffmpeg_names() {
    assert_eq!(
        frame_spans(&[(3, 9), (20, 20)]),
        vec![
            FrameSpan { first: 3, last: 9 },
            FrameSpan {
                first: 20,
                last: 20
            }
        ]
    );
    assert_eq!(encoder_name(LocalizedEncoder::X264), "libx264");
    assert_eq!(encoder_name(LocalizedEncoder::Nvenc), "h264_nvenc");
}

const RECT: PixelRect = PixelRect {
    x: 64,
    y: 48,
    width: 96,
    height: 64,
};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tbd-localize-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("patches")).unwrap();
    dir
}

/// A `seconds`-long 320x180 test pattern at 24 fps with a tone in AAC, its video encoded with
/// `video_args`.
fn source(dir: &Path, seconds: u32, video_args: &[&str]) -> PathBuf {
    let video = dir.join("source.mkv");
    let status = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-y",
            "-f",
            "lavfi",
        ])
        .arg("-i")
        .arg(format!("testsrc2=size=320x180:rate=24:duration={seconds}"))
        .args(["-f", "lavfi", "-i"])
        .arg(format!("sine=frequency=440:duration={seconds}"))
        .args(video_args)
        .args(["-c:a", "aac"])
        .arg(&video)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg {video_args:?}");
    video
}

/// One baked occurrence with an opaque patch on frames `first..=last` of `frames`.
fn replacement(dir: &Path, frames: u64, first: u64, last: u64) -> ReplacementDocument {
    RgbaImage::from_pixel(RECT.width, RECT.height, Rgba([200, 40, 60, 255]))
        .save(dir.join("patches/0.png"))
        .unwrap();
    let plate = Plate {
        first_frame: first,
        last_frame: last,
        rect: RECT,
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("source.png"),
        mask: PathBuf::from("mask.png"),
        plate: None,
        patch: Some(PathBuf::from("patches/0.png")),
        shifted: Vec::new(),
    };
    ReplacementDocument {
        width: 320,
        height: 180,
        frame_count: frames,
        texts: vec![ReplacedText {
            id: "T1".into(),
            first_frame: first,
            last_frame: last,
            status: ReplaceStatus::Baked,
            style: None,
            container: None,
            plates: vec![plate],
            preview: None,
            lettering_quad: None,
        }],
    }
}

/// Render `video` with a patch on `first..=last` into `dir`; the result and the output.
fn render_patch(dir: &Path, video: &Path, first: u64, last: u64) -> (Rendered, PathBuf) {
    render_with(dir, video, Some((first, last)))
}

/// Render `video` into `dir` with a patch on the frames of `patched`, if any.
fn render_with(dir: &Path, video: &Path, patched: Option<(u64, u64)>) -> (Rendered, PathBuf) {
    let programs = Programs::default();
    let stream = media_io::probe::probe(&programs, video)
        .unwrap()
        .video
        .unwrap();
    let frames = timeline(&programs, video, stream.start_time_s, 24.0)
        .unwrap()
        .len() as u64;
    let document = match patched {
        Some((first, last)) => replacement(dir, frames, first, last),
        None => ReplacementDocument::default(),
    };
    let output = dir.join("source.localized.mkv.part");
    let pieces = dir.join("pieces");
    let rendered = render(
        &RenderRequest {
            programs: &programs,
            video,
            stream: &stream,
            document: &document,
            motion: &Motion::default(),
            root: dir,
            output: &output,
            encoder: LocalizedEncoder::X264,
            pieces_dir: &pieces,
            cancel: None,
        },
        &|_, _| {},
    )
    .unwrap();
    assert!(!pieces.exists(), "the pieces' folder is removed");
    assert_eq!(rendered.frames, frames);
    (rendered, output)
}

/// The MD5 of every decoded frame of `video`, in presentation order; the decode must be clean.
fn decoded_frames(video: &Path) -> Vec<String> {
    let output = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-i"])
        .arg(video)
        .args(["-map", "0:v:0", "-f", "framemd5", "-"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stderr).trim(), "");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.rsplit(',').next().map(|md5| md5.trim().to_string()))
        .collect()
}

fn has_audio(video: &Path) -> bool {
    let probe = media_io::probe::probe(&Programs::default(), video).unwrap();
    !probe.audio.is_empty()
}

#[test]
fn only_the_segment_with_a_patch_is_re_encoded_and_the_rest_decodes_unchanged() {
    segment_path("segment-path", "yuv420p");
}

#[test]
fn a_ten_bit_source_re_encodes_its_segment_from_one_shared_decoder() {
    segment_path("segment-path-10", "yuv420p10le");
}

/// Render a patch on frames 60–70 of a six-second `pix_fmt` source with an IDR every 24 frames
/// and check that only the second-second segment was re-encoded.
fn segment_path(name: &str, pix_fmt: &str) {
    let dir = scratch(name);
    // B-frames, and CAVLC with one reference frame, so the segment's headers differ from the
    // source's.
    let video = source(
        &dir,
        6,
        &[
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-x264-params",
            "cabac=0:ref=1:bframes=2:keyint=24:min-keyint=24:scenecut=0",
            "-pix_fmt",
            pix_fmt,
        ],
    );
    let (rendered, output) = render_patch(&dir, &video, 60, 70);
    assert_eq!(rendered.encoder, "libx264");
    assert_eq!(
        rendered.segments,
        SegmentSummary {
            segments_reencoded: 1,
            frames_reencoded: 24,
            frames_copied: 120,
            fallback_reason: None,
        }
    );
    let before = decoded_frames(&video);
    let after = decoded_frames(&output);
    assert_eq!(before.len(), 144);
    assert_eq!(after.len(), 144, "every frame is kept");
    for (index, (source, localized)) in before.iter().zip(&after).enumerate() {
        if (48..72).contains(&index) {
            if (60..=70).contains(&index) {
                assert_ne!(source, localized, "frame {index} carries the patch");
            }
        } else {
            assert_eq!(source, localized, "frame {index} is copied unchanged");
        }
    }
    assert!(has_audio(&output), "the audio stream is kept");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn without_a_patch_every_frame_is_copied() {
    let dir = scratch("copy-only");
    let video = source(
        &dir,
        2,
        &["-c:v", "libx264", "-preset", "fast", "-pix_fmt", "yuv420p"],
    );
    let (rendered, output) = render_with(&dir, &video, None);
    assert_eq!(rendered.encoder, "copy");
    assert_eq!(
        rendered.segments,
        SegmentSummary {
            segments_reencoded: 0,
            frames_reencoded: 0,
            frames_copied: 48,
            fallback_reason: None,
        }
    );
    assert_eq!(decoded_frames(&output), decoded_frames(&video));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_source_that_is_not_h264_is_encoded_whole_with_the_reason() {
    let dir = scratch("not-h264");
    let video = source(&dir, 2, &["-c:v", "mpeg4", "-q:v", "3"]);
    let (rendered, output) = render_patch(&dir, &video, 10, 20);
    assert_eq!(rendered.encoder, "libx264", "this FFmpeg has no NVENC");
    let reason = rendered.segments.fallback_reason.clone().unwrap();
    assert!(reason.contains("mpeg4, not H.264"), "{reason}");
    assert_eq!(rendered.segments, whole_summary(48, FallbackReason(reason)));
    assert_eq!(decoded_frames(&output).len(), 48);
    assert!(has_audio(&output));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_patch_in_a_source_with_one_idr_is_encoded_whole_with_the_reason() {
    let dir = scratch("one-idr");
    let video = source(
        &dir,
        2,
        &["-c:v", "libx264", "-preset", "fast", "-pix_fmt", "yuv420p"],
    );
    let (rendered, output) = render_patch(&dir, &video, 10, 20);
    assert_eq!(
        rendered.segments.fallback_reason.as_deref(),
        Some("every frame lies in a re-encoded segment")
    );
    assert_eq!(rendered.segments.frames_reencoded, 48);
    let after = decoded_frames(&output);
    assert_eq!(after.len(), 48);
    std::fs::remove_dir_all(&dir).unwrap();
}

//! FFmpeg round trips: a source is planned, cut, partly re-encoded, joined and checked.

use std::path::PathBuf;

use job_model::onscreen::LocalizedEncoder;

use super::*;
use crate::encode::EncoderProcess;
use crate::video_frames::{Decode, FrameStream, PixelFormat, timeline};

const DEADLINE: Duration = Duration::from_secs(120);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("segments-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Run FFmpeg quietly with `args`, overwriting `output`.
fn ffmpeg(args: &[&str], output: &Path) {
    let result = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error"])
        .args(args)
        .arg("-y")
        .arg(output)
        .timeout(DEADLINE)
        .output()
        .unwrap();
    assert_eq!(result.code, 0, "{}", result.stderr);
}

/// A six-second 320x180 H.264 source at 24000/1001 fps with a tone in AAC, whose encoder primes
/// the audio so it starts before the video. Its x264 settings (CAVLC, one reference frame, level
/// 3.0) make headers that differ from any segment's, which uses CABAC.
fn cavlc_source(dir: &Path) -> PathBuf {
    let video = dir.join("source.mkv");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=24000/1001:duration=6",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=6",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-x264-params",
            "cabac=0:ref=1:bframes=2:b-pyramid=none:keyint=24:min-keyint=24:scenecut=0",
            "-level",
            "3.0",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
        ],
        &video,
    );
    video
}

/// The pieces a source's changed frames make, cut and encoded by `encoder` into `folder`, with
/// each re-encoded frame inverted; the source's probe, timeline and pieces.
fn render_pieces(
    video: &Path,
    changed: &[FrameSpan],
    folder: &Path,
    encoder: LocalizedEncoder,
) -> (H264Source, Vec<(f64, f64)>, Vec<Piece>) {
    let programs = Programs::default();
    let source = probe_h264_source(&programs, video).unwrap();
    let frames = timeline(&programs, video, 0.0, source.fps()).unwrap();
    let keys = keyframe_packets(&programs, video).unwrap();
    let indices: Vec<u64> = keys.iter().map(|key| key.index).collect();
    segment_eligibility(&source, &frames, &indices).unwrap();
    let format = source.packet_format.unwrap();
    let mut probe = IdrProbe::new(&programs, video, format, keys, source.frame_s());
    let pieces = plan_pieces(changed, &indices, frames.len() as u64, &mut |batch| {
        probe.confirm(batch)
    })
    .unwrap();
    copy_pieces(&programs, video, &pieces, folder, DEADLINE, None).unwrap();
    let size = (source.width, source.height);
    let mut stream = FrameStream::open_native(
        &programs,
        video,
        size,
        0.0,
        source.fps(),
        PixelFormat::Yuv420p,
        Decode::Exact,
    )
    .unwrap();
    for (number, piece) in pieces.iter().enumerate() {
        let Piece::Encode { first, last } = *piece else {
            continue;
        };
        let file = piece_file(folder, number, *piece);
        let spec = SegmentSpec::for_source(&source, PixelFormat::Yuv420p, encoder, file).unwrap();
        let mut encoder = EncoderProcess::start_segment(&programs, &spec, DEADLINE, None).unwrap();
        loop {
            let frame = stream.next_frame().unwrap().unwrap();
            if frame.index < first {
                continue;
            }
            let inverted: Vec<u8> = frame.rgb.iter().map(|value| 255 - value).collect();
            encoder.write_frame(&inverted).unwrap();
            if frame.index == last {
                break;
            }
        }
        assert_eq!(encoder.finish().unwrap(), piece.frames());
    }
    drop(stream);
    (source, frames, pieces)
}

/// Join `pieces` in `folder` with the video offset `offset_s` into `name` there.
fn join(
    video: &Path,
    source: &H264Source,
    pieces: &[Piece],
    folder: &Path,
    offset_s: f64,
    name: &str,
) -> PathBuf {
    let output = folder.join(name);
    let request = JoinRequest {
        folder: folder.to_path_buf(),
        source: video.to_path_buf(),
        video_offset_s: offset_s,
        frame_rate: source.frame_rate,
        output: output.clone(),
    };
    join_pieces(&Programs::default(), pieces, &request, DEADLINE, None).unwrap();
    output
}

fn verify(
    video: &Path,
    source: &H264Source,
    frames: &[(f64, f64)],
    pieces: &[Piece],
    output: &Path,
    folder: &Path,
) -> Result<(), FallbackReason> {
    verify_join(
        &Programs::default(),
        &VerifyRequest {
            source: video,
            source_timeline: frames,
            source_starts: source.starts,
            frame_rate: source.frame_rate,
            pieces,
            output,
            folder,
            timeout: DEADLINE,
        },
    )
}

/// The MD5 of every decoded frame of `video`, in presentation order.
fn decoded_frames(video: &Path) -> Vec<String> {
    let output = Run::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-i"])
        .arg(video)
        .args(["-map", "0:v:0", "-f", "framemd5", "-"])
        .timeout(DEADLINE)
        .output()
        .unwrap();
    assert_eq!(output.code, 0, "{}", output.stderr);
    assert_eq!(output.stderr.trim(), "", "the decode complained");
    output
        .stdout
        .lines()
        .filter(|line| !line.starts_with('#'))
        .filter_map(|line| line.rsplit(',').next().map(|md5| md5.trim().to_string()))
        .collect()
}

/// The stream headers (`extradata`) of `video`'s first video stream as hex.
fn headers(video: &Path) -> String {
    let output = Run::new("ffprobe")
        .args(["-v", "error", "-select_streams", "v:0", "-show_streams"])
        .args([
            "-show_data",
            "-show_entries",
            "stream=extradata",
            "-of",
            "default",
        ])
        .arg(video)
        .timeout(DEADLINE)
        .output()
        .unwrap();
    output.stdout
}

#[test]
fn a_segment_with_different_headers_joins_cleanly_and_copies_the_rest_unchanged() {
    segment_round_trip(LocalizedEncoder::X264, "headers");
}

#[test]
#[ignore = "needs FFmpeg with NVENC; run on the host"]
fn an_nvenc_segment_joins_cleanly_and_copies_the_rest_unchanged() {
    segment_round_trip(LocalizedEncoder::Nvenc, "nvenc");
}

/// One segment of frames 24–47 re-encoded by `encoder` in a CAVLC source, joined and checked:
/// only those frames change.
fn segment_round_trip(encoder: LocalizedEncoder, name: &str) {
    let dir = scratch(name);
    let video = cavlc_source(&dir);
    let (source, frames, pieces) = render_pieces(
        &video,
        &[FrameSpan {
            first: 30,
            last: 40,
        }],
        &dir,
        encoder,
    );
    assert_eq!(
        pieces,
        [
            Piece::Copy { first: 0, last: 23 },
            Piece::Encode {
                first: 24,
                last: 47
            },
            Piece::Copy {
                first: 48,
                last: 143
            },
        ]
    );
    let segment = piece_file(&dir, 1, pieces[1]);
    assert_ne!(
        headers(&segment),
        headers(&video),
        "the headers must differ"
    );
    let output = join(
        &video,
        &source,
        &pieces,
        &dir,
        frames[0].0,
        "joined.mkv.part",
    );
    assert_eq!(
        verify(&video, &source, &frames, &pieces, &output, &dir),
        Ok(())
    );
    let before = decoded_frames(&video);
    let after = decoded_frames(&output);
    assert_eq!(before.len(), 144);
    assert_eq!(after.len(), 144);
    for (index, (source_frame, joined_frame)) in before.iter().zip(&after).enumerate() {
        let encoded = (24..48).contains(&index);
        assert_eq!(source_frame != joined_frame, encoded, "frame {index}");
    }
    let summary = piece_summary(&pieces);
    assert_eq!((summary.frames_reencoded, summary.frames_copied), (24, 120));
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pieces_cut_without_in_band_headers_fail_the_decode_check() {
    let dir = scratch("headerless");
    let video = cavlc_source(&dir);
    let (source, frames, pieces) = render_pieces(
        &video,
        &[FrameSpan {
            first: 30,
            last: 40,
        }],
        &dir,
        LocalizedEncoder::X264,
    );
    // Cut the copied pieces again as a plain stream copy: their packets lack the headers.
    let pattern = dir.join("copy%05d.mkv");
    let video_arg = video.to_string_lossy().into_owned();
    ffmpeg(
        &[
            "-i",
            &video_arg,
            "-map",
            "0:v:0",
            "-c",
            "copy",
            "-f",
            "segment",
            "-segment_format",
            "matroska",
            "-segment_frames",
            "24,48",
        ],
        &pattern,
    );
    let output = join(
        &video,
        &source,
        &pieces,
        &dir,
        frames[0].0,
        "headerless.mkv.part",
    );
    let reason = verify(&video, &source, &frames, &pieces, &output, &dir).unwrap_err();
    assert!(reason.0.contains("does not decode cleanly"), "{reason}");
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn an_offset_source_keeps_its_video_start_and_a_wrong_offset_fails_the_sync_check() {
    let dir = scratch("offset");
    let plain = dir.join("plain.mkv");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=320x180:rate=24:duration=4",
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-x264-params",
            "bframes=2:keyint=24:min-keyint=24:scenecut=0",
            "-pix_fmt",
            "yuv420p",
        ],
        &plain,
    );
    let tone = dir.join("tone.mka");
    ffmpeg(
        &[
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=5",
            "-c:a",
            "flac",
        ],
        &tone,
    );
    let video = dir.join("offset.mkv");
    let (plain_arg, tone_arg) = (
        plain.to_string_lossy().into_owned(),
        tone.to_string_lossy().into_owned(),
    );
    ffmpeg(
        &[
            "-itsoffset",
            "0.5",
            "-i",
            &plain_arg,
            "-i",
            &tone_arg,
            "-map",
            "0",
            "-map",
            "1",
            "-c",
            "copy",
        ],
        &video,
    );
    let (source, frames, pieces) = render_pieces(
        &video,
        &[FrameSpan {
            first: 50,
            last: 52,
        }],
        &dir,
        LocalizedEncoder::X264,
    );
    let offset = source.starts.video_after_audio_s().unwrap();
    assert!((offset - 0.5).abs() < 0.002, "{offset}");
    assert!((frames[0].0 - 0.5).abs() < 0.002, "{:?}", frames[0]);
    let output = join(
        &video,
        &source,
        &pieces,
        &dir,
        frames[0].0,
        "joined.mkv.part",
    );
    assert_eq!(
        verify(&video, &source, &frames, &pieces, &output, &dir),
        Ok(())
    );
    let joined = stream_starts(&Programs::default(), &output).unwrap();
    assert!((joined.video_after_audio_s().unwrap() - 0.5).abs() < 0.002);
    let wrong = join(&video, &source, &pieces, &dir, 0.0, "wrong.mkv.part");
    let reason = verify(&video, &source, &frames, &pieces, &wrong, &dir).unwrap_err();
    assert!(reason.0.contains("after its audio"), "{reason}");
    std::fs::remove_dir_all(dir).unwrap();
}

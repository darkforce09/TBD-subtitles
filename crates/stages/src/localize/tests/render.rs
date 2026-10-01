use std::path::{Path, PathBuf};
use std::process::Command;

use image::{Rgba, RgbaImage};
use job_model::onscreen::{
    LocalizedEncoder, PixelRect, Plate, ReplaceStatus, ReplacedText, ReplacementDocument,
};
use job_model::outputs::VideoStream;
use media_io::video_frames::{Decode, FrameStream, PixelFormat, timeline};

use super::*;

fn with_pix_fmt(name: Option<&str>) -> VideoStream {
    VideoStream {
        pix_fmt: name.map(String::from),
        ..VideoStream::default()
    }
}

#[test]
fn ten_bit_420_sources_decode_as_ten_bit_and_everything_else_as_eight_bit() {
    assert_eq!(
        frame_format(&with_pix_fmt(Some("yuv420p10le"))),
        PixelFormat::Yuv420p10le
    );
    assert_eq!(
        frame_format(&with_pix_fmt(Some("p010le"))),
        PixelFormat::Yuv420p10le
    );
    for other in [Some("yuv420p"), Some("yuv422p10le"), Some("rgb24"), None] {
        assert_eq!(frame_format(&with_pix_fmt(other)), PixelFormat::Yuv420p);
    }
}

#[test]
fn a_stream_without_a_frame_rate_is_refused_before_anything_runs() {
    let stream = VideoStream {
        width: 64,
        height: 64,
        ..VideoStream::default()
    };
    let request = RenderRequest {
        programs: &Programs::default(),
        video: Path::new("/nonexistent/source.mkv"),
        stream: &stream,
        document: &ReplacementDocument::default(),
        motion: &Motion::default(),
        root: Path::new("/nonexistent"),
        output: Path::new("/nonexistent/out.mkv"),
        encoder: LocalizedEncoder::X264,
        pieces_dir: Path::new("/nonexistent/pieces"),
        cancel: None,
    };
    let error = render(&request, &|_, _| {}).unwrap_err();
    assert!(error.0.contains("frame rate"), "{error}");
}

const PATCH_RGB: [u8; 3] = [200, 40, 60];
const RECT: PixelRect = PixelRect {
    x: 65,
    y: 47,
    width: 96,
    height: 64,
};

fn ffmpeg(args: &[&str]) {
    let status = Command::new("ffmpeg")
        .args(["-nostdin", "-hide_banner", "-v", "error", "-y"])
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "ffmpeg {args:?}");
}

/// A two-second 320x240 test pattern in `pix_fmt` with a tone and a subtitle track.
fn source_video(dir: &Path, pix_fmt: &str) -> PathBuf {
    let subtitles = dir.join("source.srt");
    std::fs::write(&subtitles, "1\n00:00:00,000 --> 00:00:01,500\nHello\n").unwrap();
    let video = dir.join("source.mkv");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc=size=320x240:rate=24:duration=2",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:duration=2",
        "-i",
        &subtitles.to_string_lossy(),
        "-map",
        "0",
        "-map",
        "1",
        "-map",
        "2",
        "-c:v",
        "libx264",
        "-pix_fmt",
        pix_fmt,
        "-c:a",
        "aac",
        "-c:s",
        "srt",
        &video.to_string_lossy(),
    ]);
    video
}

fn replacement(frame_count: u64) -> ReplacementDocument {
    let plate = Plate {
        first_frame: 10,
        last_frame: 30,
        rect: RECT,
        shift: [0.0, 0.0],
        scale: 1.0,
        source: PathBuf::from("visual/plates/T1/source.png"),
        mask: PathBuf::from("visual/plates/T1/mask.png"),
        plate: None,
        patch: Some(PathBuf::from("visual/patches/T1/0.png")),
        shifted: Vec::new(),
    };
    ReplacementDocument {
        width: 320,
        height: 240,
        frame_count,
        texts: vec![ReplacedText {
            id: "T1".into(),
            first_frame: 10,
            last_frame: 30,
            status: ReplaceStatus::Baked,
            style: None,
            container: None,
            plates: vec![plate],
            preview: None,
            lettering_quad: None,
        }],
    }
}

/// Frames `wanted` of `video` as rgb24.
fn rgb_frames(programs: &Programs, video: &Path, wanted: &[u64]) -> Vec<Vec<u8>> {
    let mut stream =
        FrameStream::open(programs, video, (320, 240), 0.0, 24.0, Decode::Exact).unwrap();
    let mut frames = Vec::new();
    while let Some(frame) = stream.next_frame().unwrap() {
        if wanted.contains(&frame.index) {
            frames.push(frame.rgb);
        }
    }
    stream.finish().unwrap();
    frames
}

/// The mean of each channel over the 4x4 block at (x, y).
fn block_mean(rgb: &[u8], x: u32, y: u32) -> [f64; 3] {
    let mut sum = [0.0; 3];
    for dy in 0..4 {
        for dx in 0..4 {
            let at = (((y + dy) * 320 + x + dx) * 3) as usize;
            for channel in 0..3 {
                sum[channel] += f64::from(rgb[at + channel]);
            }
        }
    }
    sum.map(|total| total / 16.0)
}

fn close(a: [f64; 3], b: [f64; 3], tolerance: f64) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= tolerance)
}

#[test]
#[ignore = "needs FFmpeg"]
fn render_blends_the_patch_into_its_frames_and_keeps_the_audio_without_subtitles() {
    render_case("yuv420p", PixelFormat::Yuv420p);
}

#[test]
#[ignore = "needs FFmpeg"]
fn render_blends_a_ten_bit_source_in_ten_bits() {
    render_case("yuv420p10le", PixelFormat::Yuv420p10le);
}

/// Render a patch into a test video encoded in `pix_fmt`, decoded as `format`, and check the
/// result.
fn render_case(pix_fmt: &str, format: PixelFormat) {
    let dir = std::env::temp_dir().join(format!(
        "tbd-localize-render-{pix_fmt}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("visual/patches/T1")).unwrap();
    let programs = Programs::default();
    let video = source_video(&dir, pix_fmt);
    let probe = media_io::probe::probe(&programs, &video).unwrap();
    let stream = probe.video.unwrap();
    assert_eq!(frame_format(&stream), format);
    let frames = timeline(&programs, &video, stream.start_time_s, 24.0)
        .unwrap()
        .len();
    let [r, g, b] = PATCH_RGB;
    RgbaImage::from_pixel(RECT.width, RECT.height, Rgba([r, g, b, 255]))
        .save(dir.join("visual/patches/T1/0.png"))
        .unwrap();
    let document = replacement(frames as u64);
    let output = dir.join("source.localized.mkv.part");
    let reports = std::sync::Mutex::new(Vec::new());
    let rendered = render(
        &RenderRequest {
            programs: &programs,
            video: &video,
            stream: &stream,
            document: &document,
            motion: &Motion::default(),
            root: &dir,
            output: &output,
            encoder: LocalizedEncoder::X264,
            pieces_dir: &dir.join("pieces"),
            cancel: None,
        },
        &|done, total| reports.lock().unwrap().push((done, total)),
    )
    .unwrap();
    eprintln!("encoded with {}: {:?}", rendered.encoder, rendered.segments);
    assert!(!dir.join("pieces").exists(), "the work folder is removed");
    assert_eq!(rendered.frames, frames as u64);
    assert_eq!(reports.lock().unwrap().last(), Some(&(frames, frames)));

    let out_frames = timeline(&programs, &output, 0.0, 24.0).unwrap();
    assert_eq!(out_frames.len(), frames);
    let out_probe = media_io::probe::probe(&programs, &output).unwrap();
    assert!(!out_probe.audio.is_empty(), "the audio stream is kept");
    let out_stream = out_probe.video.as_ref().unwrap();
    assert_eq!(frame_format(out_stream), format, "{:?}", out_stream.pix_fmt);
    let subtitle_streams = Command::new("ffprobe")
        .args(["-v", "error", "-select_streams", "s", "-show_entries"])
        .args(["stream=index", "-of", "csv=p=0"])
        .arg(&output)
        .output()
        .unwrap();
    assert!(subtitle_streams.status.success());
    assert!(
        String::from_utf8_lossy(&subtitle_streams.stdout)
            .trim()
            .is_empty(),
        "no subtitle stream"
    );
    let source_rgb = rgb_frames(&programs, &video, &[5, 20]);
    let output_rgb = rgb_frames(&programs, &output, &[5, 20]);
    let patch = PATCH_RGB.map(f64::from);
    let (inside_x, inside_y) = (RECT.x + 40, RECT.y + 30);
    let inside = block_mean(&output_rgb[1], inside_x, inside_y);
    assert!(close(inside, patch, 12.0), "inside {inside:?}");
    for (x, y) in [(8, 200), (280, 8), (inside_x, inside_y)] {
        let before = block_mean(&source_rgb[0], x, y);
        let after = block_mean(&output_rgb[0], x, y);
        assert!(
            close(before, after, 12.0),
            "frame 5 at {x},{y}: {before:?} {after:?}"
        );
    }
    for (x, y) in [(8, 200), (280, 8)] {
        let before = block_mean(&source_rgb[1], x, y);
        let after = block_mean(&output_rgb[1], x, y);
        assert!(
            close(before, after, 12.0),
            "frame 20 at {x},{y}: {before:?} {after:?}"
        );
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

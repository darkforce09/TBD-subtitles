use std::path::PathBuf;
use std::time::Duration;

use child_process::Run;

use super::*;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("pcm-stream-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A two-second 440 Hz stereo tone at 48 kHz, as FFmpeg's own sine source writes it.
fn tone(dir: &std::path::Path) -> PathBuf {
    let path = dir.join("tone.wav");
    let out = Run::new("ffmpeg")
        .args(["-nostdin", "-v", "error", "-f", "lavfi", "-i"])
        .arg("sine=frequency=440:sample_rate=48000:duration=2")
        .args(["-ac", "2"])
        .arg(&path)
        .timeout(Duration::from_secs(30))
        .output()
        .unwrap();
    assert_eq!(out.code, 0, "{}", out.stderr);
    path
}

fn request(video: &std::path::Path, format: PcmFormat) -> PcmRequest<'_> {
    PcmRequest {
        video,
        audio_position: 0,
        format,
        window: None,
        chunk_frames: 4_000,
        deadline: Duration::from_secs(30),
    }
}

#[test]
fn decodes_in_fixed_chunks_at_the_asked_rate() {
    let dir = scratch("chunks");
    let video = tone(&dir);
    let mut stream =
        PcmStream::open(&Programs::default(), &request(&video, PcmFormat::MONO_16K)).unwrap();
    let mut sizes = Vec::new();
    let mut peak = 0f32;
    while let Some(chunk) = stream.next_chunk() {
        let chunk = chunk.unwrap();
        peak = chunk.iter().fold(peak, |p, s| p.max(s.abs()));
        sizes.push(chunk.len());
    }
    stream.finish().unwrap();
    let total: usize = sizes.iter().sum();
    assert!(
        (31_900..=32_100).contains(&total),
        "{total} samples for 2 s at 16 kHz"
    );
    assert!(sizes[..sizes.len() - 1].iter().all(|&n| n == 4_000));
    assert!(peak > 0.05, "the tone is audible: peak {peak}");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_window_decodes_an_excerpt_in_stereo() {
    let dir = scratch("window");
    let video = tone(&dir);
    let mut req = request(&video, PcmFormat::STEREO_44K);
    req.window = Some((0.5, 1.0));
    let path = dir.join("excerpt.f32");
    let written =
        write_f32_file(PcmStream::open(&Programs::default(), &req).unwrap(), &path).unwrap();
    assert!(
        (88_000..=88_400).contains(&written),
        "{written} samples for 1 s of stereo 44.1 kHz"
    );
    assert_eq!(F32FileReader::sample_count(&path).unwrap(), written);
    let mut reader = F32FileReader::open(&path, 10_000).unwrap();
    let mut read = 0u64;
    while let Some(chunk) = reader.next_chunk().unwrap() {
        read += chunk.len() as u64;
    }
    assert_eq!(read, written);
    let middle = read_f32_range(&path, 1000, 500).unwrap();
    assert_eq!(middle.len(), 500);
    let tail = read_f32_range(&path, written - 10, 500).unwrap();
    assert_eq!(tail.len(), 10, "a range past the end is cut short");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_missing_file_fails_at_finish() {
    let missing = PathBuf::from("/nonexistent/video.mp4");
    let stream = PcmStream::open(
        &Programs::default(),
        &request(&missing, PcmFormat::MONO_16K),
    )
    .unwrap();
    assert!(matches!(stream.finish(), Err(MediaError::Exit { .. })));
}

#[test]
fn little_endian_bytes_become_samples() {
    let bytes: Vec<u8> = [1.0f32, -0.5]
        .iter()
        .flat_map(|s| s.to_le_bytes())
        .collect();
    assert_eq!(samples_from_le(&bytes), vec![1.0, -0.5]);
}

//! Frames for the visual scan: a screening copy in presentation order and full-resolution stills.
//!
//! **Role:** hand the scan every frame of a small proxy copy with its presentation interval, and
//! decode single source frames by index when the scan confirms its keyframes.
//! **Position:** between `media_io::video_frames` and the scan; tests substitute scripted sources.
//! **Signals and state:** one proxy `FrameStream` until `finish_proxies`, a copy of the video's
//! timeline, and at most four concurrent still decoders.
//! **Invariants:** proxies arrive in presentation order, one per timeline entry; a still is the
//! source frame at its timeline time, at source size; an unknown index is an error; the source
//! video is only read.

use std::path::{Path, PathBuf};

use image::RgbImage;
use job_model::outputs::VideoStream;
use media_io::Programs;
use media_io::preview::frame_size;
use media_io::video_frames::{Decode, FrameStream, still};

use super::{PROXY_LINES, frame_rate};
use crate::onscreen_text::TextResult;

/// Still decoders running at once.
const STILL_DECODERS: usize = 4;

/// One frame of the screening copy with its presentation index and origin-relative interval.
pub struct ProxyFrame {
    pub index: u64,
    pub time_s: f64,
    pub end_s: f64,
    pub rgb: RgbImage,
}

/// The frames a scan reads: the proxy stream, then full-resolution stills of chosen frames.
pub trait FrameSource {
    fn proxy_size(&self) -> (u32, u32);
    /// Origin-relative (time_s, end_s) per frame index, known before decoding.
    fn timeline(&self) -> &[(f64, f64)];
    fn next_proxy(&mut self) -> TextResult<Option<ProxyFrame>>;
    /// Called once after the last proxy; fails when the decoder failed or stopped short.
    fn finish_proxies(&mut self) -> TextResult<()>;
    /// Full-resolution stills for these frame indices, in the same order.
    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>>;
}

/// A video decoded by FFmpeg: a fast proxy stream and accurately seeked stills.
pub struct FfmpegSource {
    programs: Programs,
    video: PathBuf,
    fps: f64,
    size: (u32, u32),
    proxy: (u32, u32),
    timeline: Vec<(f64, f64)>,
    stream: Option<FrameStream>,
}

impl FfmpegSource {
    /// Reads the video's timeline and starts decoding its proxy copy, `PROXY_LINES` rows high.
    pub fn open(programs: &Programs, video: &Path, stream: &VideoStream) -> TextResult<Self> {
        let proxy = frame_size(stream.width, stream.height, PROXY_LINES);
        let fps = frame_rate(stream);
        let frames = FrameStream::open(
            programs,
            video,
            proxy,
            stream.start_time_s,
            fps,
            Decode::Proxy,
        )?;
        Ok(Self {
            programs: programs.clone(),
            video: video.to_path_buf(),
            fps,
            size: (stream.width, stream.height),
            proxy,
            timeline: frames.timeline().to_vec(),
            stream: Some(frames),
        })
    }
}

impl FrameSource for FfmpegSource {
    fn proxy_size(&self) -> (u32, u32) {
        self.proxy
    }

    fn timeline(&self) -> &[(f64, f64)] {
        &self.timeline
    }

    fn next_proxy(&mut self) -> TextResult<Option<ProxyFrame>> {
        let Some(stream) = self.stream.as_mut() else {
            return Ok(None);
        };
        let Some(frame) = stream.next_frame()? else {
            return Ok(None);
        };
        let rgb = RgbImage::from_raw(self.proxy.0, self.proxy.1, frame.rgb)
            .ok_or("The proxy decoder returned a frame of the wrong size")?;
        Ok(Some(ProxyFrame {
            index: frame.index,
            time_s: frame.time_s,
            end_s: frame.end_s,
            rgb,
        }))
    }

    fn finish_proxies(&mut self) -> TextResult<()> {
        match self.stream.take() {
            Some(stream) => Ok(stream.finish()?),
            None => Ok(()),
        }
    }

    fn stills(&mut self, indices: &[u64]) -> TextResult<Vec<RgbImage>> {
        let times = indices
            .iter()
            .map(|&index| {
                usize::try_from(index)
                    .ok()
                    .and_then(|position| self.timeline.get(position))
                    .map(|&(time_s, _)| time_s)
                    .ok_or_else(|| format!("Frame {index} is not in the video's timeline").into())
            })
            .collect::<TextResult<Vec<f64>>>()?;
        let (programs, video, fps, size) =
            (&self.programs, self.video.as_path(), self.fps, self.size);
        let mut stills = Vec::with_capacity(times.len());
        for chunk in times.chunks(STILL_DECODERS) {
            let decoded: Vec<TextResult<Vec<u8>>> = std::thread::scope(|scope| {
                let workers: Vec<_> = chunk
                    .iter()
                    .map(|&time_s| {
                        scope.spawn(move || still::still(programs, video, time_s, fps, size))
                    })
                    .collect();
                workers
                    .into_iter()
                    .map(|worker| match worker.join() {
                        Ok(result) => result.map_err(Into::into),
                        Err(_) => Err("A still decoder thread panicked".into()),
                    })
                    .collect()
            });
            for rgb in decoded {
                stills.push(
                    RgbImage::from_raw(size.0, size.1, rgb?)
                        .ok_or("A still frame has the wrong size")?,
                );
            }
        }
        Ok(stills)
    }
}

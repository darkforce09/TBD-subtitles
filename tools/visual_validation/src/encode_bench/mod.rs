//! Localized-video encode benchmark.
//!
//! **Role:** on one clip of a video, time the encodes the localized video can run on raw frames
//! streamed through this process: the H.264 segment encode matching the source with each x264
//! preset of a range and each NVENC preset, and the whole-video HEVC encode with each NVENC
//! preset; print each encode's frame rate, size, bit rate and PSNR against the source as
//! Markdown tables under one header.
//! **Position:** the `encode-bench` command of the validation tool. NVENC rows need the host's
//! driver; elsewhere they say the encoder is unavailable.
//! **Signals and state:** the encoded clips in a folder of their own, removed at the end unless
//! one was named.
//! **Invariants:** the source video is only read; a configuration that fails prints its error in
//! its row and the bench goes on; the segment rows use the production segment command line with
//! only the preset changed.

mod args;
mod measure;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use job_model::onscreen::LocalizedEncoder;
use media_io::Programs;
use media_io::encode::segments::{
    H264Source, NVENC_SEGMENT_PRESET, SegmentSpec, X264_SEGMENT_PRESET, probe_h264_source,
    segment_args, segment_encoder,
};
use media_io::encode::{Encoder, available_encoder};
use media_io::video_frames::PixelFormat;
use stages::localize::frame_format;

use args::{Clip, NVENC_PRESETS, RawInput, X264_PRESETS, hevc_args};
use measure::{Row, Source, header, measure, render};

/// The whole-video HEVC encode's production preset.
const HEVC_WHOLE_PRESET: &str = "p6";

/// The `encode-bench` command's arguments.
#[derive(clap::Args, Debug, Clone, PartialEq)]
pub struct Options {
    /// The video to measure; only read.
    pub video: PathBuf,
    /// Where the clip starts, in seconds.
    #[arg(long, default_value_t = 600.0)]
    pub start: f64,
    /// How long the clip runs, in seconds.
    #[arg(long, default_value_t = 60.0)]
    pub duration: f64,
    /// The folder holding `ffmpeg` and `ffprobe`; the pair beside this binary, else `PATH`.
    #[arg(long)]
    pub ffmpeg_dir: Option<PathBuf>,
    /// Where the encoded clips are kept; a temporary folder, removed at the end, when not given.
    #[arg(long)]
    pub out_dir: Option<PathBuf>,
}

impl Options {
    /// The clip, when its start and duration are usable.
    fn clip(&self) -> Result<Clip> {
        anyhow::ensure!(
            self.start.is_finite()
                && self.start >= 0.0
                && self.duration.is_finite()
                && self.duration > 0.0,
            "the clip needs a start of at least 0 and a positive duration"
        );
        Ok(Clip {
            start_s: self.start,
            duration_s: self.duration,
        })
    }
}

/// Run every encode and print the tables.
pub fn run(options: &Options) -> Result<()> {
    let clip = options.clip()?;
    let programs = programs(options.ffmpeg_dir.as_deref())?;
    let probe = media_io::probe::probe(&programs, &options.video)?;
    let stream = probe.video.context("the file has no video stream")?;
    anyhow::ensure!(stream.frame_rate_den > 0, "the video reports no frame rate");
    let folder = match &options.out_dir {
        Some(dir) => dir.clone(),
        None => std::env::temp_dir().join(format!("encode-bench-{}", std::process::id())),
    };
    std::fs::create_dir_all(&folder).with_context(|| format!("create {}", folder.display()))?;
    let source = Source {
        programs: &programs,
        video: &options.video,
        clip,
        format: frame_format(&stream),
        size: (stream.width, stream.height),
    };
    let h264 = probe_h264_source(&programs, &options.video).map_err(|error| error.to_string());
    let nvenc_h264 = h264.as_ref().is_ok_and(|h264| {
        segment_encoder(&programs, LocalizedEncoder::Nvenc, h264) == LocalizedEncoder::Nvenc
    });
    let nvenc_hevc = matches!(available_encoder(&programs), Ok(Encoder::HevcNvenc));

    println!("# encode-bench: {}\n", file_name(&options.video));
    println!(
        "Clip {:.0}–{:.0} s of {}×{} {} ({}) at {}/{} fps, encoded from raw {} frames.  ",
        clip.start_s,
        clip.start_s + clip.duration_s,
        stream.width,
        stream.height,
        stream.codec,
        stream.pix_fmt.as_deref().unwrap_or("unknown pixel format"),
        stream.frame_rate_num,
        stream.frame_rate_den,
        source.format.name()
    );
    println!(
        "FFmpeg {}; h264_nvenc {}; hevc_nvenc {}.\n",
        programs.ffmpeg,
        yes_no(nvenc_h264),
        yes_no(nvenc_hevc)
    );

    println!(
        "## H.264 segment encode (production: x264 `{X264_SEGMENT_PRESET}`, NVENC `{NVENC_SEGMENT_PRESET}`)\n"
    );
    print!("{}", header());
    let segment_rows = X264_PRESETS
        .iter()
        .map(|preset| (LocalizedEncoder::X264, *preset, true))
        .chain(
            NVENC_PRESETS
                .iter()
                .map(|preset| (LocalizedEncoder::Nvenc, *preset, nvenc_h264)),
        );
    for (encoder, preset, available) in segment_rows {
        let row = segment_row(&source, &h264, encoder, preset, available, &folder);
        print!("{}", render(&row, clip.duration_s));
    }

    println!("\n## Whole-video HEVC encode (production: NVENC `{HEVC_WHOLE_PRESET}`)\n");
    print!("{}", header());
    let input = RawInput {
        size: source.size,
        frame_rate: (stream.frame_rate_num, stream.frame_rate_den),
        format: source.format,
    };
    for preset in NVENC_PRESETS {
        let output = folder.join(format!("hevc_nvenc-{preset}.mkv"));
        let result = if nvenc_hevc {
            let args = hevc_args(input, preset, stream.bit_rate, &output);
            measure(&source, args, &output).map_err(|error| format!("failed: {error:#}"))
        } else {
            Err("unavailable".to_string())
        };
        let row = Row {
            encoder: "hevc_nvenc".into(),
            preset: preset.into(),
            result,
        };
        print!("{}", render(&row, clip.duration_s));
    }
    if options.out_dir.is_none() {
        let _ = std::fs::remove_dir_all(&folder);
    }
    Ok(())
}

/// One segment row: `encoder` at `preset` with the production segment command line, or why it
/// cannot run.
fn segment_row(
    source: &Source,
    h264: &Result<H264Source, String>,
    encoder: LocalizedEncoder,
    preset: &str,
    available: bool,
    folder: &Path,
) -> Row {
    let name = match encoder {
        LocalizedEncoder::X264 => "libx264",
        LocalizedEncoder::Nvenc => "h264_nvenc",
    };
    let output = folder.join(format!("{name}-{preset}.mkv"));
    let result = match (h264, available) {
        (Err(error), _) => Err(format!("n/a: {error}")),
        (Ok(_), false) => Err("unavailable".to_string()),
        (Ok(h264), true) => {
            segment_spec(h264, source.format, encoder, preset, &output).and_then(|spec| {
                measure(source, segment_args(&spec), &output)
                    .map_err(|error| format!("failed: {error:#}"))
            })
        }
    };
    Row {
        encoder: name.into(),
        preset: preset.into(),
        result,
    }
}

/// The production segment encode of `h264` with `encoder` at `preset`, or why there is none: a
/// source x264 encodes alone (10-bit) or with no matching profile or level.
pub fn segment_spec(
    h264: &H264Source,
    format: PixelFormat,
    encoder: LocalizedEncoder,
    preset: &str,
    output: &Path,
) -> Result<SegmentSpec, String> {
    let mut spec = SegmentSpec::for_source(h264, format, encoder, output.to_path_buf())
        .map_err(|reason| format!("n/a: {reason}"))?;
    if spec.encoder != encoder {
        return Err("n/a: the source's profile has no NVENC segment".to_string());
    }
    spec.preset = preset.to_string();
    Ok(spec)
}

/// The FFmpeg pair in `dir`, or the one beside this binary (else on `PATH`) without it.
fn programs(dir: Option<&Path>) -> Result<Programs> {
    let Some(dir) = dir else {
        return Ok(Programs::beside_current_exe());
    };
    let (ffmpeg, ffprobe) = (dir.join("ffmpeg"), dir.join("ffprobe"));
    anyhow::ensure!(
        ffmpeg.is_file() && ffprobe.is_file(),
        "{} lacks ffmpeg or ffprobe",
        dir.display()
    );
    Ok(Programs {
        ffmpeg: ffmpeg.to_string_lossy().into_owned(),
        ffprobe: ffprobe.to_string_lossy().into_owned(),
        bundled: true,
    })
}

fn yes_no(available: bool) -> &'static str {
    if available {
        "available"
    } else {
        "unavailable"
    }
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

#[cfg(test)]
#[path = "tests/options.rs"]
mod tests;

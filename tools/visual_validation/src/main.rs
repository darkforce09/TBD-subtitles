//! Reproducible visual translation validation on owner-provided media.
//!
//! **Role:** fetch pinned models, recognize stills and evaluate annotated video artifacts.
//! **Position:** repository tool calling the same backends and stages as production.
//! **Signals and state:** local input files and explicit JSON reports; no source mutation.
//! **Invariants:** missing readable occurrences fail; uncertain tracks require explicit fallback.

mod evaluate;
mod pilot;
mod scenarios;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use inference::{
    model_store,
    ocr::{OcrDetector, OcrReader},
};
use job_model::onscreen::*;
use std::ops::ControlFlow;
use std::path::PathBuf;

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate an eight-second Japanese source fixture with independent annotations.
    Scenarios { output: PathBuf },
    /// Run the six production visual stages on a pilot or benchmark, with isolated workers.
    Run {
        video: PathBuf,
        output: PathBuf,
        #[arg(long, default_value = "target/release")]
        binaries: PathBuf,
        #[arg(long)]
        dialogue: Option<PathBuf>,
        #[arg(long)]
        claude: bool,
    },
    /// Inspect a font candidate before adding its checksum to the pinned manifest.
    FontCandidate { url: String, output: PathBuf },
    /// Download the checksum-pinned OCR models through the production store.
    Fetch,
    /// Recognize a still with the production local detector and readers (run on the host).
    Image { image: PathBuf, output: PathBuf },
    /// Validate a visual document against hand-annotated occurrences and frame geometry.
    Evaluate {
        actual: PathBuf,
        annotations: PathBuf,
        output: PathBuf,
    },
    /// Print compact occurrence readings, translations, confidence and review flags.
    Inspect { document: PathBuf },
    /// Extract a bounded pilot clip, preserving the input video.
    Clip {
        video: PathBuf,
        output: PathBuf,
        #[arg(long)]
        start: f64,
        #[arg(long)]
        duration: f64,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Scenarios { output } => scenarios::generate(&output)?,
        Command::Run {
            video,
            output,
            binaries,
            dialogue,
            claude,
        } => pilot::run(&video, &output, &binaries, dialogue.as_deref(), claude)?,
        Command::FontCandidate { url, output } => {
            anyhow::ensure!(
                url.starts_with("https://raw.githubusercontent.com/google/fonts/"),
                "only Google Fonts source candidates are accepted"
            );
            let mut response = ureq::get(&url).call()?;
            let mut file = std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&output)?;
            std::io::copy(&mut response.body_mut().as_reader(), &mut file)?;
            println!(
                "{} {}",
                std::fs::metadata(&output)?.len(),
                model_store::sha256_of(&output)?
            );
        }
        Command::Fetch => {
            let root = model_store::models_dir()?;
            for id in ["pp-ocrv5", "manga-ocr", "visual-font"] {
                model_store::fetch_model(&root, id, &mut |file, held, total| {
                    if held == total {
                        eprintln!("{} verified", file.file);
                    }
                    ControlFlow::Continue(())
                })?;
            }
        }
        Command::Image { image, output } => recognize(&image, &output)?,
        Command::Evaluate {
            actual,
            annotations,
            output,
        } => evaluate::run(&actual, &annotations, &output)?,
        Command::Inspect { document } => inspect(&document)?,
        Command::Clip {
            video,
            output,
            start,
            duration,
        } => {
            anyhow::ensure!(
                start.is_finite()
                    && start >= 0.0
                    && duration.is_finite()
                    && duration > 0.0
                    && duration <= 120.0,
                "pilot duration must be 0–120 seconds"
            );
            anyhow::ensure!(!output.exists(), "pilot output already exists");
            let result = child_process::Run::new(media_io::Programs::beside_current_exe().ffmpeg)
                .args([
                    "-nostdin",
                    "-v",
                    "error",
                    "-ss",
                    &start.to_string(),
                    "-i",
                    video.to_str().context("video path is not UTF-8")?,
                    "-t",
                    &duration.to_string(),
                    "-map",
                    "0:v:0",
                    "-map",
                    "0:a:0?",
                    "-c:v",
                    "libx264",
                    "-crf",
                    "16",
                    "-c:a",
                    "aac",
                    "-n",
                    output.to_str().context("output path is not UTF-8")?,
                ])
                .output()?;
            anyhow::ensure!(result.code == 0, "FFmpeg: {}", result.stderr);
        }
    }
    Ok(())
}

fn inspect(path: &std::path::Path) -> Result<()> {
    let document: TextDocument = pipeline::work_dir::read_json(path)?;
    println!(
        "{}",
        serde_json::json!({"summary":document.summary(),"width":document.width,"height":document.height,"decoded_frames":document.decoded_frames})
    );
    for item in document.occurrences {
        println!(
            "{}",
            serde_json::json!({"id":item.id,"start_s":item.start_s,"end_s":item.end_s,"japanese":item.japanese,"english":item.english,"confidence":item.confidence,"rendered":item.rendered,"warnings":item.warnings})
        );
    }
    Ok(())
}

fn recognize(path: &std::path::Path, output: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(output)?;
    let root = model_store::models_dir()?;
    let mut detector = OcrDetector::open(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut reader = OcrReader::open(&root).map_err(|e| anyhow::anyhow!("{e}"))?;
    let image = image::open(path)?.to_rgb8();
    let mut document = TextDocument {
        review_warnings: Vec::new(),
        width: image.width(),
        height: image.height(),
        decoded_frames: 1,
        occurrences: Vec::new(),
    };
    let started = std::time::Instant::now();
    for (index, (quad, score)) in detector
        .detect(&image)
        .map_err(|e| anyhow::anyhow!("{e}"))?
        .into_iter()
        .enumerate()
    {
        let crop = stages::onscreen_text::detect::crop(&image, quad);
        let (japanese, confidence) = reader.read(&crop).map_err(|e| anyhow::anyhow!("{e}"))?;
        let file = PathBuf::from(format!("crop-{index:04}.png"));
        crop.save(output.join(&file))?;
        document.occurrences.push(TextOccurrence {
            source_fingerprint: None,
            id: format!("text-{index:04}"),
            start_s: 0.0,
            end_s: 1.0,
            japanese,
            english: None,
            confidence,
            crops: vec![file],
            frames: vec![TextFrame {
                time_s: 0.0,
                end_s: 1.0,
                quad,
                confidence: score,
                surface_rgb: None,
            }],
            provenance: TextProvenance {
                backend: "PP-OCRv5 / manga-ocr".into(),
                ..TextProvenance::default()
            },
            presentation: TextPresentation::default(),
            warnings: Vec::new(),
            reviewed: false,
            rendered: None,
        });
    }
    pipeline::work_dir::write_json(&output.join("observations.json"), &document)?;
    println!(
        "{} regions in {:.2}s; {}",
        document.occurrences.len(),
        started.elapsed().as_secs_f64(),
        output.display()
    );
    Ok(())
}

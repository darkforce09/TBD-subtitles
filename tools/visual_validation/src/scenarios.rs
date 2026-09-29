//! Deterministic Japanese source scenarios for the production visual validation path.
//!
//! **Role:** create credits, lyrics, vertical writing and brief/repeated signs in eight seconds.
//! **Position:** validation-only media generation; no production inference or model outputs.
//! **Signals and state:** a new output folder, pinned Japanese font, source ASS, video and JSON.
//! **Invariants:** existing files are untouched; annotation times use source frames, and manual
//! quadrilaterals support flagged overlap checks rather than a two-pixel tracking claim.

use anyhow::{Context, Result, ensure};
use inference::model_store;
use serde_json::{Value, json};
use std::path::Path;
use std::time::Duration;

const SOURCE_ASS: &str = r"[Script Info]
Title: Independent Japanese visual validation source
ScriptType: v4.00+
PlayResX: 1920
PlayResY: 1080
WrapStyle: 2
ScaledBorderAndShadow: yes

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Source,Noto Sans JP,96,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,5,0,0,0,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:00.00,0:00:02.00,Source,,0,0,0,,{\move(960,720,960,360,0,2000)}制作
Dialogue: 0,0:00:00.00,0:00:02.00,Source,,0,0,0,,{\move(960,900,960,540,0,2000)}音楽
Dialogue: 0,0:00:02.00,0:00:04.00,Source,,0,0,0,,{\pos(960,840)\fad(250,250)}君と一緒に
Dialogue: 0,0:00:04.00,0:00:06.00,Source,,0,0,0,,{\pos(960,540)\fs108}海\N賊
Dialogue: 0,0:00:06.00,0:00:06.12,Source,,0,0,0,,{\pos(960,540)\fs112}注意
Dialogue: 0,0:00:06.50,0:00:08.00,Source,,0,0,0,,{\pos(960,540)\fs112}注意
";

pub(super) fn generate(output: &Path) -> Result<()> {
    ensure!(
        !output.exists(),
        "scenario output already exists; choose a new folder"
    );
    let font = model_store::MODEL_FILES
        .iter()
        .find(|file| file.model == "visual-font" && file.file == "NotoSansJP.ttf")
        .context("pinned Japanese font is missing from the manifest")?;
    let fonts = model_store::models_dir()?.join("visual-font");
    let font_path = fonts.join(font.file);
    ensure!(
        font_path.is_file(),
        "Japanese fixture font is unavailable; run visual_validation fetch first"
    );
    ensure!(
        model_store::sha256_of(&font_path)? == font.sha256,
        "Japanese fixture font checksum differs from its pin; download visual-font again"
    );
    std::fs::create_dir_all(output)?;
    let output = output.canonicalize()?;
    std::fs::write(output.join("fixture-japanese.ass"), SOURCE_ASS)?;
    let filter = format!(
        "drawbox=x=0:y=0:w=iw:h=ih:color=0x503018:t=fill:enable='gte(n,48)*lt(n,96)',\
         drawbox=x=0:y=0:w=iw:h=ih:color=0x123820:t=fill:enable='gte(n,96)*lt(n,144)',\
         drawbox=x=0:y=0:w=iw:h=ih:color=0x421832:t=fill:enable='gte(n,144)*lt(n,156)',\
         drawbox=x=0:y=0:w=iw:h=ih:color=0x124050:t=fill:enable='gte(n,156)',\
         ass=filename=fixture-japanese.ass:fontsdir='{}'",
        escape_filter_path(&fonts)?
    );
    let programs = media_io::Programs::beside_current_exe();
    let result = child_process::Run::new(&programs.ffmpeg)
        .cwd(&output)
        .args([
            "-nostdin",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=0x102038:s=1920x1080:r=24:d=8",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=channel_layout=stereo:sample_rate=48000:d=8",
            "-map",
            "0:v:0",
            "-map",
            "1:a:0",
            "-vf",
            &filter,
            "-c:v",
            "libx264",
            "-preset",
            "fast",
            "-crf",
            "12",
            "-threads",
            "1",
            "-pix_fmt",
            "yuv420p",
            "-r",
            "24",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-metadata:s:a:0",
            "language=eng",
            "-t",
            "8",
            "-shortest",
            "-n",
            "source.mp4",
        ])
        .timeout(Duration::from_secs(120))
        .output()?;
    ensure!(result.code == 0, "scenario FFmpeg: {}", result.stderr);
    pipeline::work_dir::write_json(&output.join("annotations.json"), &annotations())?;
    pipeline::work_dir::write_json(
        &output.join("notes.json"),
        &json!({
            "source":"Independently authored fixture-japanese.ass burned into source.mp4; its filename is distinct from production source.ass output.",
            "duration_s":8,"fps":24,"width":1920,"height":1080,"frames":192,
            "audio":"Silent stereo AAC tagged eng, permitting the normal production input path without fabricated spoken dialogue.",
            "font":{"file":font.file,"sha256":font.sha256},
            "geometry":"Approximate manually specified glyph regions. These annotations support flagged nearby overlap coverage only, not accepted two-pixel tracking accuracy.",
            "timing":"Intervals are first visible source frame through the first absent frame, end exclusive. The lyric is fully transparent at frame48 and first visible at49. ASS end6.12 removes the three-frame sign before frame147 at6.125.",
            "cases":[
                {"category":"scrolling credits","japanese":["制作","音楽"],"frames":[0,48]},
                {"category":"visible lyric with fade in and out","japanese":"君と一緒に","frames":[49,96]},
                {"category":"vertical text","japanese":"海賊","frames":[96,144],"note":"One semantic word laid out top to bottom with an ASS line break."},
                {"category":"brief three-frame appearance","japanese":"注意","frames":[144,147]},
                {"category":"repeated sign after a cut","japanese":"注意","frames":[156,192]}
            ],
            "use":"Run the ordinary visual_validation run command on source.mp4 into a separate work folder, then evaluate its resulting visual document against annotations.json. Never use fixture-japanese.ass as a reference translation."
        }),
    )?;
    println!(
        "Eight-second source and independent annotations: {}",
        output.display()
    );
    Ok(())
}

fn escape_filter_path(path: &Path) -> Result<String> {
    let path = path.to_str().context("font directory is not UTF-8")?;
    ensure!(
        !path.chars().any(|c| c.is_control()),
        "font directory contains a control character"
    );
    Ok(path
        .replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('\'', "'\\''"))
}

fn annotations() -> Value {
    json!({"fps":24.0,"height":1080,"occurrences":[
        occurrence("credit-production","制作",0.0,2.0,&[
            (0.25,[850.0,615.0,1070.0,735.0]),
            (1.5,[850.0,390.0,1070.0,510.0])]),
        occurrence("credit-music","音楽",0.0,2.0,&[
            (0.25,[850.0,795.0,1070.0,915.0]),
            (1.5,[850.0,570.0,1070.0,690.0])]),
        occurrence("fading-visible-lyric","君と一緒に",49.0/24.0,4.0,&[
            (3.0,[690.0,775.0,1230.0,905.0])]),
        occurrence("vertical-pirate","海賊",4.0,6.0,&[
            (5.0,[880.0,385.0,1040.0,695.0])]),
        occurrence("three-frame-warning","注意",6.0,147.0/24.0,&[
            (6.0,[825.0,460.0,1095.0,620.0])]),
        occurrence("warning-repeated-after-cut","注意",6.5,8.0,&[
            (7.0,[825.0,460.0,1095.0,620.0])])
    ]})
}

fn occurrence(
    label: &str,
    japanese: &str,
    start_s: f64,
    end_s: f64,
    samples: &[(f64, [f64; 4])],
) -> Value {
    let frames: Vec<_> = samples
        .iter()
        .map(|(time, [left, top, right, bottom])| {
            json!({"time_s":time,"quad":[
                {"x":left,"y":top},{"x":right,"y":top},
                {"x":right,"y":bottom},{"x":left,"y":bottom}
            ]})
        })
        .collect();
    json!({"label":label,"japanese":japanese,"start_s":start_s,"end_s":end_s,"frames":frames})
}

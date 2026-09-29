use super::*;

fn request() -> VisualPreview {
    VisualPreview {
        video: "/media/a video.mkv".into(),
        ass: Some("/media/a video.ass".into()),
        time_s: 123.456789,
        duration_s: 0.0,
        size: (960, 540),
        fps: 24_000.0 / 1001.0,
    }
}

fn value<'a>(args: &'a [String], option: &str) -> &'a str {
    let index = args.iter().position(|arg| arg == option).unwrap();
    &args[index + 1]
}

#[test]
fn subtitles_use_the_video_timestamp_and_native_size_before_preview_scaling() {
    let args = args(&request()).unwrap();
    let filter = value(&args, "-vf");
    assert!(filter.starts_with("setpts=PTS-STARTPTS+123.456789/TB,subtitles=filename="));
    assert!(filter.ends_with(",setpts=PTS-STARTPTS,scale=960:540"));
    assert_eq!(value(&args, "-ss"), "123.456789");
    assert_eq!(value(&args, "-seek_timestamp"), "0");
    assert_eq!(value(&args, "-frames:v"), "1");
    assert_eq!(value(&args, "-pix_fmt"), "rgb24");
    assert_eq!(value(&args, "-i"), "/media/a video.mkv");
    assert_eq!(args.last().unwrap(), "pipe:1");
    assert!(!args.iter().any(|arg| arg == "-t" || arg == "-copyts"));
}

#[test]
fn playback_resamples_only_after_rendering_and_limits_output_duration() {
    let mut request = request();
    request.duration_s = 2.5;
    let args = args(&request).unwrap();
    assert_eq!(value(&args, "-t"), "2.5");
    assert!(value(&args, "-vf").ends_with(&format!("scale=960:540,fps={}", request.fps)));
    assert!(!args.iter().any(|arg| arg == "-frames:v"));
}

#[test]
fn original_preview_has_the_same_timing_and_size_without_subtitle_rendering() {
    let mut request = request();
    request.ass = None;
    let args = args(&request).unwrap();
    assert_eq!(
        value(&args, "-vf"),
        "setpts=PTS-STARTPTS+123.456789/TB,setpts=PTS-STARTPTS,scale=960:540"
    );
}

#[test]
fn paths_escape_both_option_and_filtergraph_delimiters() {
    let path = Path::new("/tmp/a'b:c\\d[e],f;g.ass");
    assert_eq!(
        filter_path(path).unwrap(),
        r"/tmp/a\\\'b\\:c\\\\d\[e\]\,f\;g.ass"
    );
    assert_eq!(
        filter_path(Path::new(" leading and trailing ")).unwrap(),
        r"\\\ leading\\\ and\\\ trailing\\\ "
    );
}

#[test]
fn invalid_times_rates_sizes_and_paths_are_rejected() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        let mut bad = request();
        bad.time_s = invalid;
        assert!(args(&bad).is_err());
        bad = request();
        bad.duration_s = invalid;
        assert!(args(&bad).is_err());
        bad = request();
        bad.fps = invalid;
        assert!(args(&bad).is_err());
    }
    let mut bad = request();
    bad.fps = 0.0;
    assert!(args(&bad).is_err());
    for size in [(0, 540), (960, 0), (u32::MAX, 1), (65536, 65536)] {
        bad = request();
        bad.size = size;
        assert!(args(&bad).is_err());
    }
    bad = request();
    bad.time_s = f64::MAX;
    assert!(args(&bad).is_err());
    for path in ["", "contains\0nul"] {
        bad = request();
        bad.ass = Some(path.into());
        assert!(args(&bad).is_err());
        bad = request();
        bad.video = path.into();
        assert!(args(&bad).is_err());
    }
}

#[test]
#[ignore = "runs FFmpeg; run on the host with its ASS-enabled FFmpeg"]
fn ffmpeg_renders_a_hostile_ass_path_at_the_correct_time_after_a_nonzero_start() {
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "tbd-visual-preview-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let video = root.join("black.mkv");
    let ass = root.join("caption's: \\[x],;name.ass");
    std::fs::write(&ass, "[Script Info]\nScriptType: v4.00+\nPlayResX: 160\nPlayResY: 90\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H00FFFFFF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,0,0,7,0,0,0,1\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:02.00,0:00:02.20,Default,,0,0,0,,{\\an7\\pos(0,0)\\bord0\\shad0\\p1}m 0 0 l 80 0 80 80 0 80\n").unwrap();
    let made = Command::new("ffmpeg")
        .args([
            "-nostdin",
            "-hide_banner",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "color=c=black:s=160x90:r=10:d=4",
            "-vf",
            "setpts=PTS+7/TB",
            "-c:v",
            "ffv1",
            "-y",
        ])
        .arg(&video)
        .output()
        .unwrap();
    assert!(
        made.status.success(),
        "{}",
        String::from_utf8_lossy(&made.stderr)
    );
    let mut request = VisualPreview {
        video,
        ass: Some(ass),
        time_s: 2.0,
        duration_s: 0.0,
        size: (160, 90),
        fps: 10.0,
    };
    let render = |request: &VisualPreview| {
        let output = Command::new("ffmpeg")
            .args(args(request).unwrap())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout.len(), 160 * 90 * 3);
        output.stdout
    };
    let translated = render(&request);
    request.time_s = 1.0;
    let before = render(&request);
    request.time_s = 2.0;
    request.ass = None;
    let original = render(&request);
    assert!(translated.iter().filter(|&&value| value > 200).count() > 5_000);
    assert!(before.iter().all(|&value| value < 20));
    assert!(original.iter().all(|&value| value < 20));
    std::fs::remove_dir_all(root).unwrap();
}

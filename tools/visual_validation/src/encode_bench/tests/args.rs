use super::*;

const CLIP: Clip = Clip {
    start_s: 600.0,
    duration_s: 60.0,
};

fn joined(args: &[String]) -> String {
    args.join(" ")
}

#[test]
fn the_clip_decodes_raw_in_the_encode_s_format_with_no_rate_conversion() {
    assert_eq!(
        joined(&decode_args(
            Path::new("/videos/ep 11.mkv"),
            CLIP,
            PixelFormat::Yuv420p
        )),
        "-nostdin -hide_banner -v error -noautorotate -ss 600.000 -t 60.000 -i /videos/ep 11.mkv \
         -map 0:v:0 -an -sn -dn -fps_mode passthrough -pix_fmt yuv420p -f rawvideo pipe:1"
    );
    let ten_bit = decode_args(Path::new("ep.mkv"), CLIP, PixelFormat::Yuv420p10le);
    assert!(joined(&ten_bit).contains("-pix_fmt yuv420p10le -f rawvideo"));
}

#[test]
fn the_whole_video_rows_run_production_nvenc_hevc_at_each_preset() {
    let input = RawInput {
        size: (1920, 1080),
        frame_rate: (24000, 1001),
        format: PixelFormat::Yuv420p,
    };
    assert_eq!(
        joined(&hevc_args(
            input,
            "p3",
            Some(4_000_000),
            Path::new("/bench/hevc_nvenc-p3.mkv")
        )),
        "-hide_banner -v error -y -f rawvideo -pix_fmt yuv420p -s 1920x1080 -framerate 24000/1001 \
         -i pipe:0 -map 0:v:0 -c:v hevc_nvenc -preset p3 -tune hq -rc vbr -cq 19 -b:v 0 \
         -profile:v main -maxrate 5000000 -bufsize 10000000 -f matroska /bench/hevc_nvenc-p3.mkv"
    );
    let ten_bit = RawInput {
        format: PixelFormat::Yuv420p10le,
        ..input
    };
    let args = joined(&hevc_args(ten_bit, "p7", None, Path::new("out.mkv")));
    assert!(args.contains("-profile:v main10"), "{args}");
    assert!(!args.contains("-maxrate"), "{args}");
}

#[test]
fn the_comparison_aligns_both_clips_at_zero() {
    let args = psnr_args(Path::new("ep.mkv"), CLIP, Path::new("/bench/x.mkv"));
    assert_eq!(
        joined(&args),
        "-nostdin -hide_banner -nostats -v info -ss 600.000 -t 60.000 -i ep.mkv -i /bench/x.mkv \
         -lavfi [0:v:0]setpts=PTS-STARTPTS[source];[1:v:0]setpts=PTS-STARTPTS[encoded];\
         [encoded][source]psnr -f null -"
    );
}

#[test]
fn the_average_psnr_is_read_from_the_filter_s_line() {
    let log = "Input #0, matroska,webm, from 'ep.mkv':\n\
               [Parsed_psnr_2 @ 0x55d] PSNR y:44.123 u:47.5 v:47.9 average:45.012345 min:39.1 max:52.0\n";
    assert_eq!(parse_psnr(log), Some(45.012345));
    let identical = "[Parsed_psnr_2 @ 0x1] PSNR y:inf u:inf v:inf average:inf min:inf max:inf";
    assert_eq!(parse_psnr(identical), Some(f64::INFINITY));
    assert_eq!(parse_psnr("no comparison ran"), None);
    assert_eq!(parse_psnr("average:12.0 without the filter's name"), None);
}

#[test]
fn the_preset_ranges_run_fastest_first() {
    assert_eq!(X264_PRESETS.first(), Some(&"ultrafast"));
    assert_eq!(X264_PRESETS.last(), Some(&"veryslow"));
    assert!(
        X264_PRESETS.contains(&"slow"),
        "the production preset is measured"
    );
    assert_eq!(NVENC_PRESETS, ["p1", "p2", "p3", "p4", "p5", "p6", "p7"]);
}

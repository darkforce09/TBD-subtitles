use super::*;

const CLIP: Clip = Clip {
    start_s: 600.0,
    duration_s: 120.0,
};

#[test]
fn the_nvdec_route_decodes_on_the_gpu_and_downloads_nv12() {
    let args = Route::NvdecNv12Wide.args(Path::new("/media/ep.mp4"), CLIP, (1920, 1080));
    let joined = args.join(" ");
    assert!(joined.starts_with(
        "-nostdin -hide_banner -v error -noautorotate -hwaccel cuda -hwaccel_output_format cuda \
         -ss 600 -i /media/ep.mp4 -t 120"
    ));
    assert!(joined.ends_with("-vf hwdownload,format=nv12 -pix_fmt nv12 -f rawvideo pipe:1"));
}

#[test]
fn rgb_routes_scale_to_the_frame_size_and_yuv_routes_pass_through() {
    let rgb = Route::Rgb24
        .args(Path::new("v.mp4"), CLIP, (1920, 1080))
        .join(" ");
    assert!(rgb.contains("-vf scale=1920:1080 -pix_fmt rgb24"));
    let yuv = Route::Yuv420pWide
        .args(Path::new("v.mp4"), CLIP, (1920, 1080))
        .join(" ");
    assert!(yuv.contains("-vf null -pix_fmt yuv420p"));
}

#[test]
fn frame_sizes_follow_the_pixel_format() {
    assert_eq!(Route::Rgb24Wide.frame_bytes((1920, 1080)), 6_220_800);
    assert_eq!(Route::NvdecNv12Wide.frame_bytes((1920, 1080)), 3_110_400);
}

#[test]
fn the_wide_pipe_never_exceeds_one_mebibyte() {
    let size = wide_pipe();
    assert!(size > 0 && size <= WIDE_PIPE);
}

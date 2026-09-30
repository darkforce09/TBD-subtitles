use job_model::outputs::VideoStream;
use media_io::video_frames::PixelFormat;

use super::*;

fn stream(height: u32, space: Option<&str>, range: Option<&str>) -> VideoStream {
    VideoStream {
        width: height * 16 / 9,
        height,
        color_space: space.map(String::from),
        color_range: range.map(String::from),
        ..VideoStream::default()
    }
}

fn eight(matrix: Matrix, range: Range) -> Conversion {
    Conversion {
        matrix,
        range,
        bits: 8,
    }
}

#[test]
fn bt709_limited_maps_black_white_red_and_blue_to_their_studio_values() {
    let conversion = eight(Matrix::Bt709, Range::Limited);
    assert_eq!(conversion.samples([0, 0, 0]), [16, 128, 128]);
    assert_eq!(conversion.samples([255, 255, 255]), [235, 128, 128]);
    assert_eq!(conversion.samples([255, 0, 0]), [63, 102, 240]);
    assert_eq!(conversion.samples([0, 0, 255]), [32, 240, 118]);
}

#[test]
fn bt709_full_uses_the_whole_byte() {
    let conversion = eight(Matrix::Bt709, Range::Full);
    assert_eq!(conversion.samples([0, 0, 0]), [0, 128, 128]);
    assert_eq!(conversion.samples([255, 255, 255]), [255, 128, 128]);
    assert_eq!(conversion.samples([255, 0, 0]), [54, 99, 255]);
}

#[test]
fn bt601_limited_maps_red_green_and_blue_to_the_standard_values() {
    let conversion = eight(Matrix::Bt601, Range::Limited);
    assert_eq!(conversion.samples([255, 0, 0]), [81, 90, 240]);
    assert_eq!(conversion.samples([0, 255, 0]), [145, 54, 34]);
    assert_eq!(conversion.samples([0, 0, 255]), [41, 240, 110]);
}

#[test]
fn ten_bit_limited_scales_the_studio_range_by_four() {
    let conversion = Conversion {
        matrix: Matrix::Bt709,
        range: Range::Limited,
        bits: 10,
    };
    assert_eq!(conversion.samples([0, 0, 0]), [64, 512, 512]);
    assert_eq!(conversion.samples([255, 255, 255]), [940, 512, 512]);
    assert_eq!(conversion.max_sample(), 1023);
}

#[test]
fn the_matrix_follows_the_tag_else_the_height() {
    assert_eq!(Matrix::of(&stream(1080, None, None)), Matrix::Bt709);
    assert_eq!(Matrix::of(&stream(720, None, None)), Matrix::Bt709);
    assert_eq!(Matrix::of(&stream(480, None, None)), Matrix::Bt601);
    assert_eq!(
        Matrix::of(&stream(1080, Some("bt470bg"), None)),
        Matrix::Bt601
    );
    assert_eq!(
        Matrix::of(&stream(1080, Some("smpte170m"), None)),
        Matrix::Bt601
    );
    assert_eq!(Matrix::of(&stream(480, Some("bt709"), None)), Matrix::Bt709);
    assert_eq!(
        Matrix::of(&stream(2160, Some("bt2020nc"), None)),
        Matrix::Bt2020
    );
}

#[test]
fn the_range_is_limited_unless_tagged_full() {
    assert_eq!(Range::of(&stream(1080, None, None)), Range::Limited);
    assert_eq!(Range::of(&stream(1080, None, Some("tv"))), Range::Limited);
    assert_eq!(Range::of(&stream(1080, None, Some("pc"))), Range::Full);
}

#[test]
fn the_bit_depth_follows_the_frame_format() {
    let hd = stream(1080, None, None);
    assert_eq!(Conversion::of(&hd, PixelFormat::Yuv420p).bits, 8);
    assert_eq!(Conversion::of(&hd, PixelFormat::Yuv420p10le).bits, 10);
}

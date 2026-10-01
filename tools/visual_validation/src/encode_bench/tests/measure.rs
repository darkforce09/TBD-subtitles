use super::*;

fn row(result: Result<Measure, String>) -> Row {
    Row {
        encoder: "libx264".into(),
        preset: "slow".into(),
        result,
    }
}

#[test]
fn a_measured_row_gives_fps_size_rate_and_psnr() {
    let measured = row(Ok(Measure {
        frames: 1438,
        seconds: 12.5,
        bytes: 31_457_280,
        psnr_db: Some(44.567),
    }));
    assert_eq!(
        render(&measured, 60.0),
        "| libx264 | slow | 115.0 | 30.0 | 4.19 | 44.57 |\n"
    );
    let unmeasured = row(Ok(Measure {
        frames: 10,
        seconds: 0.0,
        bytes: 0,
        psnr_db: Some(f64::INFINITY),
    }));
    assert_eq!(
        render(&unmeasured, 60.0),
        "| libx264 | slow | n/a | 0.0 | 0.00 | inf |\n"
    );
}

#[test]
fn a_failed_row_shows_why_in_place_of_its_figures() {
    assert_eq!(
        render(&row(Err("unavailable".into())), 60.0),
        "| libx264 | slow | unavailable |  |  |  |\n"
    );
    assert_eq!(
        render(&row(Err("failed: a | b\nc".into())), 60.0),
        "| libx264 | slow | failed: a \\| b c |  |  |  |\n"
    );
}

#[test]
fn the_header_names_six_columns() {
    let header = header();
    assert!(header.starts_with("| Encoder | Preset | fps | Size (MiB) | Mbit/s | PSNR (dB) |\n"));
    assert!(header.ends_with("|---|---|---|---|---|---|\n"));
}

#[test]
fn the_last_line_of_a_log_says_what_failed() {
    assert_eq!(last_line("a\nInvalid level\n\n"), "Invalid level");
    assert_eq!(last_line(""), "no message");
}

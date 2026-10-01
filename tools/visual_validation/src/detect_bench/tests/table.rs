use super::*;

#[test]
fn a_table_renders_its_header_separator_and_rows() {
    let mut table = Table::new(&["Route", "Frames/s"]);
    table.row(vec!["rgb24".into(), "812.4".into()]);
    table.row(vec!["nv12".into()]);
    assert_eq!(
        table.render(),
        "| Route | Frames/s |\n|---|---|\n| rgb24 | 812.4 |\n| nv12 |  |\n"
    );
}

#[test]
fn a_cell_with_pipes_or_line_breaks_keeps_the_table_whole() {
    let mut table = Table::new(&["Note"]);
    table.row(vec!["error: a|b\nnext line".into(), "dropped".into()]);
    assert_eq!(
        table.render(),
        "| Note |\n|---|\n| error: a\\|b next line |\n"
    );
}

#[test]
fn an_unmeasured_value_reads_not_available() {
    assert_eq!(optional(Some(12.345), 1), "12.3");
    assert_eq!(optional(None, 1), "n/a");
    assert_eq!(optional(Some(f64::NAN), 1), "n/a");
}

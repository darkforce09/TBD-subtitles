use super::*;

#[test]
fn a_table_has_its_header_and_one_line_per_row() {
    let rows = [Row {
        scenario: "1".to_string(),
        holder: "rw, idle".to_string(),
        attempt: "ro".to_string(),
        result: "opened in 0.3 ms; counter=0".to_string(),
    }];
    assert_eq!(
        render(&rows),
        "| scenario | holder | attempt | result |\n|---|---|---|---|\n\
         | 1 | rw, idle | ro | opened in 0.3 ms; counter=0 |\n"
    );
}

#[test]
fn pipes_and_line_breaks_cannot_break_a_cell() {
    assert_eq!(
        escape_cell("failed in 0.1 ms: already open. | DatabaseAlreadyOpen\nnext"),
        "failed in 0.1 ms: already open. \\| DatabaseAlreadyOpen next"
    );
}

//! The matrix result table: one row per observation, rendered as Markdown.
//!
//! **Role:** keeps the rows the scenarios record and prints them as a Markdown table whose
//! cells hold the exact texts.
//!
//! **Position:** used by the matrix scenarios; depends on the standard library only.
//!
//! **Signals and state:** none.
//!
//! **Invariants:** a cell never breaks the table: `|` is escaped as `\|` and a line break
//! becomes a space; the texts are otherwise verbatim.

/// One observation of one scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Row {
    pub(crate) scenario: String,
    pub(crate) holder: String,
    pub(crate) attempt: String,
    pub(crate) result: String,
}

/// The table with its header: `| scenario | holder | attempt | result |`.
pub(crate) fn render(rows: &[Row]) -> String {
    let mut out = String::from("| scenario | holder | attempt | result |\n|---|---|---|---|\n");
    for row in rows {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            escape_cell(&row.scenario),
            escape_cell(&row.holder),
            escape_cell(&row.attempt),
            escape_cell(&row.result)
        ));
    }
    out
}

/// A text made safe for one Markdown table cell.
pub(crate) fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|").replace(['\r', '\n'], " ")
}

#[cfg(test)]
#[path = "tests/table.rs"]
mod tests;

//! Markdown tables for the detection benchmark's printout.
//!
//! **Role:** collect a header and rows of cells and render them as one GitHub-flavoured Markdown
//! table; format optional measurements.
//! **Position:** used by every section of `detect-bench`.
//! **Signals and state:** a table holds its own cells until rendered.
//! **Invariants:** a cell never breaks the table: pipes are escaped and line breaks become spaces;
//! a short row is padded with empty cells.

/// A Markdown table under construction.
pub struct Table {
    header: Vec<String>,
    rows: Vec<Vec<String>>,
}

impl Table {
    /// An empty table with these column names.
    pub fn new(header: &[&str]) -> Table {
        Table {
            header: header.iter().map(|name| name.to_string()).collect(),
            rows: Vec::new(),
        }
    }

    /// Add one row; cells beyond the header are dropped, missing ones left empty.
    pub fn row(&mut self, cells: Vec<String>) {
        self.rows.push(cells);
    }

    /// The table as Markdown, one line per row, ending with a line break.
    pub fn render(&self) -> String {
        let columns = self.header.len();
        let line = |cells: &[String]| {
            let mut text = String::from("|");
            for index in 0..columns {
                let cell = cells.get(index).map(String::as_str).unwrap_or("");
                text.push(' ');
                text.push_str(&escape(cell));
                text.push_str(" |");
            }
            text.push('\n');
            text
        };
        let mut text = line(&self.header);
        text.push('|');
        text.push_str(&"---|".repeat(columns));
        text.push('\n');
        for row in &self.rows {
            text.push_str(&line(row));
        }
        text
    }
}

/// `cell` with its pipes escaped and its line breaks turned into spaces.
fn escape(cell: &str) -> String {
    cell.replace('|', "\\|").replace(['\r', '\n'], " ")
}

/// `value` with `decimals` decimals, or `n/a` when it was not measured.
pub fn optional(value: Option<f64>, decimals: usize) -> String {
    match value {
        Some(value) if value.is_finite() => format!("{value:.decimals$}"),
        _ => "n/a".to_string(),
    }
}

#[cfg(test)]
#[path = "tests/table.rs"]
mod tests;

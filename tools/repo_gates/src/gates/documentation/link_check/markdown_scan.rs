//! A Markdown document read the way a renderer reads it, for the link rules.
//!
//! **Role:** the block pass of the scan. It walks the lines once, sets aside what never renders
//! as a link (front matter, fenced and indented code blocks, HTML comments), groups the rest into
//! paragraph and heading runs, reads the reference definitions, and hands every run to the inline
//! pass ([`super::markdown_inlines`]) together with the defined labels.
//!
//! **Position:** the first step of the link-check pipeline: [`super::super::link_check`] scans
//! each judged document once and hands the [`ScannedDocument`] to every rule; the anchors of a
//! linked document come from the same scan ([`super::heading_anchors`]).
//!
//! **Signals and state:** none held; a scan is a pure function of the text.
//!
//! **Invariants:** nothing inside front matter, a fenced or indented code block, an HTML comment
//! or an inline code span is a link or a heading; a fence nested in a list item is found at the
//! item's content column; line numbers are 1-based; a reference definition is recorded once, at
//! its own line, however many uses it has, and the first definition of a label wins.

use std::collections::BTreeSet;

use super::link_destination::{Definition, definition};
use super::markdown_inlines::{InlineScan, normalise_label};
use super::markdown_lines::{
    CODE_INDENT, DEEPEST_BLOCK_INDENT, atx_heading, expand_leading_tabs, front_matter_end,
    is_setext_underline, is_thematic_break, leading_spaces, list_item, opens_block,
    strip_blockquote,
};
use crate::gate_run::markdown_fences::{self, Fence};

/// One destination the document links to: an inline link's or image's, an autolink's, or a
/// reference definition's, the one place a reference link's destination is judged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ScannedLink {
    /// The 1-based line the destination starts on.
    pub(super) line: usize,
    /// The destination with backslash escapes resolved and angle brackets removed.
    pub(super) destination: String,
}

/// A full or collapsed reference whose label no definition in the document carries.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct UndefinedReference {
    /// The 1-based line of the label.
    pub(super) line: usize,
    /// The label as written.
    pub(super) label: String,
}

/// A heading and its rendered text: inline markup removed, code span content kept.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Heading {
    /// The 1-based line the heading starts on.
    pub(super) line: usize,
    pub(super) text: String,
}

/// An inline code span.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CodeSpan {
    /// The 1-based line the span opens on.
    pub(super) line: usize,
    /// The span's content, line endings as spaces.
    pub(super) text: String,
}

/// A fenced code block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct CodeBlock {
    /// The 1-based line of the opening fence.
    pub(super) line: usize,
    /// The fence's info string.
    pub(super) info: String,
    /// The lines between the fences, container indentation removed.
    pub(super) lines: Vec<String>,
}

/// Everything the link rules read from one document.
#[derive(Debug, Default)]
pub(super) struct ScannedDocument {
    /// Every destination, in document order; reference definitions sit at their own line.
    pub(super) links: Vec<ScannedLink>,
    pub(super) undefined_references: Vec<UndefinedReference>,
    /// Every heading, in document order, the input of the heading slugs.
    pub(super) headings: Vec<Heading>,
    /// Every `id` or `name` an `<a>` tag declares.
    pub(super) explicit_anchors: Vec<String>,
    /// Every inline code span, in document order, for the rules that judge commands and paths.
    pub(super) code_spans: Vec<CodeSpan>,
    /// Every fenced code block, in document order, for the rule that judges commands.
    pub(super) code_blocks: Vec<CodeBlock>,
}

/// Scan a whole document.
pub(super) fn scan(text: &str) -> ScannedDocument {
    let mut pass = BlockPass::default();
    let lines: Vec<&str> = text.lines().collect();
    let body_start = front_matter_end(&lines);
    for (index, line) in lines.iter().enumerate().skip(body_start) {
        pass.line(index + 1, &expand_leading_tabs(line));
    }
    pass.finish()
}

/// A paragraph or heading whose lines the inline pass reads as one text.
#[derive(Debug)]
struct TextRun {
    /// `(line number, text)` pairs, leading indentation removed.
    lines: Vec<(usize, String)>,
    heading: bool,
    /// The container column the run's first line sits at.
    column: usize,
}

/// A fenced block still waiting for its closing fence.
#[derive(Debug)]
struct OpenFence {
    fence: Fence,
    column: usize,
    block: CodeBlock,
}

/// The state of the block pass between lines.
#[derive(Debug, Default)]
struct BlockPass {
    document: ScannedDocument,
    runs: Vec<TextRun>,
    definitions: Vec<(usize, Definition)>,
    paragraph: Option<TextRun>,
    /// The content column of every open list item, innermost last.
    list_columns: Vec<usize>,
    fence: Option<OpenFence>,
    in_indented_code: bool,
    in_html_comment: bool,
}

impl BlockPass {
    /// Classify one line, blockquote markers removed.
    fn line(&mut self, number: usize, raw: &str) {
        let content = strip_blockquote(raw);
        if self.in_html_comment {
            self.in_html_comment = !content.contains("-->");
            return;
        }
        if self.continue_fence(content) {
            return;
        }
        if content.trim().is_empty() {
            self.end_paragraph();
            return;
        }
        let indent = leading_spaces(content);
        self.close_list_items(content, indent);
        let column = self.list_columns.last().copied().unwrap_or(0);
        let relative = indent.saturating_sub(column);
        if self.in_indented_code && relative >= CODE_INDENT {
            return;
        }
        self.in_indented_code = false;
        if relative >= CODE_INDENT && self.paragraph.is_none() {
            self.in_indented_code = true;
            return;
        }
        let body = &content[indent.min(column)..];
        self.block(number, body, column);
    }

    /// Feed a line to the open fenced block; `false` when no block is open or the line lies
    /// outside the block's container, which closes it.
    fn continue_fence(&mut self, content: &str) -> bool {
        let Some(open) = self.fence.as_mut() else {
            return false;
        };
        let indent = leading_spaces(content);
        if !content.trim().is_empty() && indent < open.column {
            self.close_fence();
            return false;
        }
        let body = &content[indent.min(open.column)..];
        if markdown_fences::closes(open.fence, body) {
            self.close_fence();
        } else {
            open.block.lines.push(body.to_string());
        }
        true
    }

    /// Close every list item the line sits outside of. A line that continues an open paragraph
    /// lazily keeps the items open; a line that opens a block of its own cannot be lazy.
    fn close_list_items(&mut self, content: &str, indent: usize) {
        let lazy = self.paragraph.is_some() && !opens_block(content.trim());
        while self
            .list_columns
            .last()
            .is_some_and(|column| indent < *column)
        {
            if lazy {
                break;
            }
            self.list_columns.pop();
        }
    }

    /// Classify a line's body, indented at most [`DEEPEST_BLOCK_INDENT`] past its container, by
    /// the block it opens or continues.
    fn block(&mut self, number: usize, body: &str, column: usize) {
        let indent = leading_spaces(body);
        let trimmed = body.trim();
        if indent <= DEEPEST_BLOCK_INDENT {
            if let Some((fence, info)) = markdown_fences::opening(body) {
                self.end_paragraph();
                self.fence = Some(OpenFence {
                    fence,
                    column,
                    block: CodeBlock {
                        line: number,
                        info: info.to_string(),
                        lines: Vec::new(),
                    },
                });
                return;
            }
            if let Some(comment) = trimmed.strip_prefix("<!--") {
                self.end_paragraph();
                self.in_html_comment = !comment.contains("-->");
                return;
            }
            if let Some(text) = atx_heading(trimmed) {
                self.end_paragraph();
                self.runs.push(TextRun {
                    lines: vec![(number, text.to_string())],
                    heading: true,
                    column,
                });
                return;
            }
            if self.underlines_paragraph(trimmed, column) {
                return;
            }
            if is_thematic_break(trimmed) {
                self.end_paragraph();
                return;
            }
            if trimmed.starts_with('|') {
                self.end_paragraph();
                self.runs.push(TextRun {
                    lines: vec![(number, trimmed.to_string())],
                    heading: false,
                    column,
                });
                return;
            }
            if self.list_item_line(number, body, column) {
                return;
            }
            if self.paragraph.is_none()
                && let Some(found) = definition(trimmed)
            {
                self.definitions.push((number, found));
                return;
            }
        }
        self.paragraph_line(number, trimmed, column);
    }

    /// A setext underline turns the open paragraph of the same container into a heading.
    fn underlines_paragraph(&mut self, trimmed: &str, column: usize) -> bool {
        match self.paragraph.take() {
            Some(mut paragraph) if is_setext_underline(trimmed) && paragraph.column == column => {
                paragraph.heading = true;
                self.runs.push(paragraph);
                true
            }
            other => {
                self.paragraph = other;
                false
            }
        }
    }

    /// A list item opens a container at its content column; its first line starts a paragraph.
    fn list_item_line(&mut self, number: usize, body: &str, column: usize) -> bool {
        let indent = leading_spaces(body);
        let Some(item) = list_item(&body[indent..]) else {
            return false;
        };
        let rest = body[indent + item.content_offset..].trim_end();
        if self.paragraph.is_some() && (rest.trim().is_empty() || !item.interrupts_paragraph) {
            return false;
        }
        self.end_paragraph();
        let content_column = column + indent + item.content_offset;
        self.list_columns.push(content_column);
        if !rest.trim().is_empty() {
            self.block(number, rest, content_column);
        }
        true
    }

    /// Continue the open paragraph, or start one.
    fn paragraph_line(&mut self, number: usize, text: &str, column: usize) {
        match self.paragraph.as_mut() {
            Some(paragraph) => paragraph.lines.push((number, text.to_string())),
            None => {
                self.paragraph = Some(TextRun {
                    lines: vec![(number, text.to_string())],
                    heading: false,
                    column,
                });
            }
        }
    }

    fn end_paragraph(&mut self) {
        if let Some(paragraph) = self.paragraph.take() {
            self.runs.push(paragraph);
        }
    }

    fn close_fence(&mut self) {
        if let Some(open) = self.fence.take() {
            self.document.code_blocks.push(open.block);
        }
    }

    /// Close what is still open and run the inline pass over every run.
    fn finish(mut self) -> ScannedDocument {
        self.end_paragraph();
        self.close_fence();
        let mut labels: BTreeSet<String> = BTreeSet::new();
        for (number, found) in &self.definitions {
            if labels.insert(normalise_label(&found.label)) {
                self.document.links.push(ScannedLink {
                    line: *number,
                    destination: found.destination.clone(),
                });
            }
        }
        for run in &self.runs {
            let inline = InlineScan::run(&run.lines, &labels);
            if run.heading {
                self.document.headings.push(Heading {
                    line: run.lines[0].0,
                    text: inline.rendered,
                });
            }
            self.document.links.extend(inline.links);
            self.document
                .undefined_references
                .extend(inline.undefined_references);
            self.document.explicit_anchors.extend(inline.anchors);
            self.document.code_spans.extend(inline.code_spans);
        }
        self.document.links.sort_by_key(|link| link.line);
        self.document.headings.sort_by_key(|heading| heading.line);
        self.document
    }
}

#[cfg(test)]
#[path = "tests/markdown_scan.rs"]
mod tests;

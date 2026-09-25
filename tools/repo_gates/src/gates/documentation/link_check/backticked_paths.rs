//! The backticked-path rule: a repository path written as inline code in a live document names
//! something in the checkout.
//!
//! **Role:** reads every inline code span of a live document ([`read_code_span`]) and judges the
//! spans that spell a path under a top-level folder of the repository. Such a path must name a
//! tracked file, a folder that holds one, or a path git ignores (runtime output such as a build
//! tree or a work directory).
//! A span that is a pattern rather than one path — a glob, a placeholder, a set, a variable, a
//! command, a URL or an elision — is counted and skipped.
//!
//! **Position:** a [`DocumentRule`] of the link-check pipeline in [`super::super::link_check`]; it
//! reads the code spans of the scan ([`super::markdown_scan`]) and the run's tracked tree, and
//! settles every path the tree does not hold in one batch through an [`IgnoreRules`] source
//! ([`super::git_ignore_rules`]) when the run finishes.
//!
//! **Signals and state:** the paths waiting for the ignore batch and the counts; both live for one
//! run.
//!
//! **Invariants:** frozen records are never judged; a span is read as a repository path only when
//! its first segment is a tracked top-level folder; a trailing
//! `/` asks for a folder; a failed ignore batch is one "did not run" verdict for every waiting
//! path, never a pass or a break; every break names the span as written.

use std::collections::BTreeSet;

use verification_core::{Kind, Verdict};

use super::git_ignore_rules::IgnoreRules;
use super::judged_documents::DocumentArea;
use super::markdown_scan::CodeSpan;
use super::{BreakRule, DocumentRule, JudgedDocument, RuleContext, RuleFindings};

/// Why a code span that starts like a repository path is skipped instead of judged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PatternKind {
    /// `*`, `?` or `[`: a glob.
    Glob,
    /// `<` or `>`: a `<…>` placeholder, or shell redirection.
    Placeholder,
    /// `{` or `}`: a `{a,b}` set.
    Set,
    /// `$`: an environment variable.
    Variable,
    /// Whitespace: a command with its arguments, or prose.
    Command,
    /// `scheme://`: a URL.
    Url,
    /// `...` or `…`: an elided path.
    Elision,
}

/// A path a code span names, ready to judge.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct CitedPath {
    /// The repository-relative path with its line suffix and fragment removed and its `.`, `..`
    /// and empty segments resolved; `None` when `..` climbs above the repository root.
    pub(super) path: Option<String>,
    /// Whether the span ends in `/`, which asks for a folder.
    pub(super) folder: bool,
}

/// What one inline code span is to the rule.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum CodeSpanReading {
    /// No repository path: the span holds no `/`, or its first segment is no top-level folder.
    NotARepositoryPath,
    /// A repository path shape that names no single location.
    Pattern(PatternKind),
    /// A repository path to judge.
    Path(CitedPath),
}

/// Read one inline code span; `is_top_level_folder` answers for its first path segment.
pub(super) fn read_code_span(
    text: &str,
    is_top_level_folder: impl Fn(&str) -> bool,
) -> CodeSpanReading {
    let text = text.trim();
    let Some((first, _)) = text.split_once('/') else {
        return CodeSpanReading::NotARepositoryPath;
    };
    if !is_top_level_folder(first) {
        return CodeSpanReading::NotARepositoryPath;
    }
    if let Some(kind) = pattern_kind(text) {
        return CodeSpanReading::Pattern(kind);
    }
    let without_fragment = text.split_once('#').map_or(text, |(path, _)| path);
    let written_path = strip_line_suffix(without_fragment);
    CodeSpanReading::Path(CitedPath {
        path: normalise(written_path),
        folder: written_path.ends_with('/'),
    })
}

/// The kind of pattern `text` is, or `None` when it spells one path. Whitespace and a URL scheme
/// are read first, so a command or a URL is never mistaken for a glob by its `?` or `*`.
fn pattern_kind(text: &str) -> Option<PatternKind> {
    if text.contains(char::is_whitespace) {
        return Some(PatternKind::Command);
    }
    if text.contains("://") {
        return Some(PatternKind::Url);
    }
    if text.contains(['*', '?', '[']) {
        return Some(PatternKind::Glob);
    }
    if text.contains(['<', '>']) {
        return Some(PatternKind::Placeholder);
    }
    if text.contains(['{', '}']) {
        return Some(PatternKind::Set);
    }
    if text.contains('$') {
        return Some(PatternKind::Variable);
    }
    (text.contains("...") || text.contains('…')).then_some(PatternKind::Elision)
}

/// `text` without a trailing `:N`, `:N-M` or `:N:M` line suffix.
fn strip_line_suffix(text: &str) -> &str {
    let Some((path, suffix)) = text.split_once(':') else {
        return text;
    };
    let number = |part: &str| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
    let lines = match suffix.split_once(['-', ':']) {
        Some((first, last)) => number(first) && number(last),
        None => number(suffix),
    };
    if lines { path } else { text }
}

/// `path` with empty and `.` segments dropped and each `..` removing the segment before it;
/// `None` when a `..` climbs above the repository root.
fn normalise(path: &str) -> Option<String> {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                segments.pop()?;
            }
            name => segments.push(name),
        }
    }
    Some(segments.join("/"))
}

/// A path the tracked tree does not hold, waiting for the ignore batch.
#[derive(Debug)]
struct WaitingPath {
    document: String,
    line: usize,
    /// The code span as written.
    written: String,
    /// The normalised path.
    path: String,
    /// Whether the span asks for a folder.
    folder: bool,
}

impl WaitingPath {
    /// What the batch asks git about this path. A folder span is asked as a folder (`path/`);
    /// any other span both as a plain path and as a folder, because git matches a folder-only
    /// ignore pattern against a path without `/` only while that folder exists on disk, and the
    /// answer must not depend on what a checkout has built.
    fn questions(&self) -> Vec<String> {
        let as_folder = format!("{}/", self.path);
        if self.folder {
            vec![as_folder]
        } else {
            vec![self.path.clone(), as_folder]
        }
    }

    /// The path as the break names it when it differs from the span as written.
    fn checked_as(&self) -> Option<String> {
        let checked = if self.folder {
            format!("{}/", self.path)
        } else {
            self.path.clone()
        };
        (checked != self.written).then_some(checked)
    }
}

/// How the rule's judged paths ended.
#[derive(Debug, Default)]
struct PathCounts {
    tracked: usize,
    ignored: usize,
    naming_nothing: usize,
    unchecked: usize,
    patterns: usize,
}

/// The backticked-path rule's state for one run.
pub(super) struct BacktickedPaths<'s> {
    ignore_rules: &'s dyn IgnoreRules,
    waiting: Vec<WaitingPath>,
    counts: PathCounts,
}

impl<'s> BacktickedPaths<'s> {
    /// A rule that asks `ignore_rules` about the paths the tracked tree does not hold.
    pub(super) fn new(ignore_rules: &'s dyn IgnoreRules) -> BacktickedPaths<'s> {
        BacktickedPaths {
            ignore_rules,
            waiting: Vec::new(),
            counts: PathCounts::default(),
        }
    }

    /// Judge the path `span` cites against the tracked tree; hold it for the ignore batch when
    /// the tree does not hold it.
    fn judge_path(
        &mut self,
        document: &str,
        span: &CodeSpan,
        cited: CitedPath,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) {
        let written = span.text.trim();
        let Some(path) = cited.path else {
            self.counts.naming_nothing += 1;
            findings.broke(
                document,
                span.line,
                BreakRule::BacktickedPathNamesNothing,
                format!("`{written}` climbs above the repository root"),
            );
            return;
        };
        let tree = context.tree;
        if tree.is_folder(&path) || (!cited.folder && tree.is_file(&path)) {
            self.counts.tracked += 1;
        } else {
            self.waiting.push(WaitingPath {
                document: document.to_string(),
                line: span.line,
                written: written.to_string(),
                path,
                folder: cited.folder,
            });
        }
    }
}

impl DocumentRule for BacktickedPaths<'_> {
    fn judges(&self, area: DocumentArea) -> bool {
        !area.is_frozen()
    }

    fn judge(
        &mut self,
        document: &JudgedDocument<'_>,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) {
        let top_level = context.tree.children("").map(|root| &root.folders);
        let is_top_level_folder =
            |first: &str| top_level.is_some_and(|folders| folders.contains(first));
        for span in &document.scan.code_spans {
            match read_code_span(&span.text, is_top_level_folder) {
                CodeSpanReading::NotARepositoryPath => {}
                CodeSpanReading::Pattern(_) => self.counts.patterns += 1,
                CodeSpanReading::Path(cited) => {
                    self.judge_path(document.path, span, cited, context, findings);
                }
            }
        }
    }

    fn finish(&mut self, _context: &RuleContext<'_>, findings: &mut RuleFindings) {
        let waiting = std::mem::take(&mut self.waiting);
        if waiting.is_empty() {
            return;
        }
        let asked: Vec<String> = waiting
            .iter()
            .flat_map(WaitingPath::questions)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let ignored = match self.ignore_rules.ignored(&asked) {
            Ok(ignored) => ignored,
            Err(cause) => {
                self.counts.unchecked += waiting.len();
                let what = format!(
                    "{} backticked path(s) in live documents could not be checked against git's \
                     ignore rules",
                    waiting.len()
                );
                findings.did_not_run(Verdict::did_not_run(what, Kind::Ban, cause));
                return;
            }
        };
        for path in waiting {
            if path
                .questions()
                .iter()
                .any(|question| ignored.contains(question))
            {
                self.counts.ignored += 1;
                continue;
            }
            self.counts.naming_nothing += 1;
            let checked = path
                .checked_as()
                .map(|checked| format!(" (checked as `{checked}`)"))
                .unwrap_or_default();
            findings.broke(
                &path.document,
                path.line,
                BreakRule::BacktickedPathNamesNothing,
                format!("`{}`{checked}", path.written),
            );
        }
    }

    fn totals(&self) -> Vec<String> {
        let counts = &self.counts;
        let judged = counts.tracked + counts.ignored + counts.naming_nothing + counts.unchecked;
        vec![format!(
            "  backticked paths: {judged} judged in live documents — {} tracked, {} ignored by \
             git, {} naming nothing, {} unchecked; {} \
             pattern(s) skipped",
            counts.tracked,
            counts.ignored,
            counts.naming_nothing,
            counts.unchecked,
            counts.patterns
        )]
    }
}

#[cfg(test)]
#[path = "tests/backticked_paths.rs"]
mod tests;

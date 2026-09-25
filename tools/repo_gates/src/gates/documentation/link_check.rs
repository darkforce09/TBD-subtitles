//! The link check: every link in the judged Markdown reaches what it names, and every path and
//! command a live document writes as code exists.
//!
//! **Role:** the `link-check` gate and the rule pipeline it runs. Each judged document is read
//! and scanned once ([`markdown_scan`]); every registered [`DocumentRule`] that judges the
//! document's area then reports its breaks; when every document is judged, each rule settles the
//! checks it batched across the run; the gate turns the breaks into one verdict per document and
//! prints totals by rule and by area.
//!
//! **Position:** `cargo gates link-check [--report] [--path <dir>]... [--with-untracked]`
//! calls [`verify_link_check`]. The judged files and their areas come from [`judged_documents`]. The
//! rules are the link rule, [`link_targets::LinkTargets`]; the backticked-path rule, [`backticked_paths::BacktickedPaths`], which
//! asks git's ignore rules through [`git_ignore_rules`]; and the command-citation rule,
//! [`command_citations::CommandCitations`], which walks the gate runner's own clap command tree.
//! The tracked tree, the scope and the report come from [`crate::gate_run`].
//!
//! **Signals and state:** none held between runs; a run owns its rules, whose caches and batched
//! checks last until the run prints.
//!
//! **Invariants:** a rule plugs in by implementing [`DocumentRule`] and joining the list in
//! [`verify_link_check`], without touching the scan; the frozen records are judged only by rules
//! whose [`DocumentRule::judges`] accepts them; a document that cannot be read, a target a rule
//! cannot read, and a batch that fails are each "did not run" (exit 2), never a pass; every break
//! prints as `path:line: rule: message`.

mod backticked_paths;
mod command_citations;
mod git_ignore_rules;
mod heading_anchors;
mod inline_html;
mod judged_documents;
mod link_destination;
mod link_targets;
mod markdown_inlines;
mod markdown_lines;
mod markdown_scan;
mod target_resolution;

use std::collections::BTreeMap;
use std::path::Path;

use verification_core::{Finding, Kind, NotRun, Verdict};

use crate::gate_run::tracked_tree::TrackedTree;
use crate::gate_run::{GateRequest, GateRun, judged_nothing, prepare, read_tracked, scope_line};
use backticked_paths::BacktickedPaths;
use command_citations::{CommandCitations, gates_command_tree};
use git_ignore_rules::GitIgnoreRules;
use judged_documents::{DocumentArea, judged_area};
use link_targets::LinkTargets;
use markdown_scan::{ScannedDocument, scan};

/// The gate's name on its header and summary lines.
const GATE: &str = "link-check";

/// How many breaks a run without `--report` prints in full.
const FIRST_BREAKS: usize = 20;

/// How much of the break list a run prints.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BreakListing {
    /// The totals, every failing document, and the first [`FIRST_BREAKS`] breaks in full.
    First,
    /// Every break in full (`--report`).
    Every,
}

/// The rule a break violates, named on every break line and counted in the totals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum BreakRule {
    MissingTarget,
    EscapesRepository,
    UndefinedReference,
    MissingAnchor,
    LineAnchorOutOfRange,
    BacktickedPathNamesNothing,
    CitedCommandDoesNotExist,
}

impl BreakRule {
    /// Every rule, in the order the totals list them.
    const ALL: [BreakRule; 7] = [
        BreakRule::MissingTarget,
        BreakRule::EscapesRepository,
        BreakRule::UndefinedReference,
        BreakRule::MissingAnchor,
        BreakRule::LineAnchorOutOfRange,
        BreakRule::BacktickedPathNamesNothing,
        BreakRule::CitedCommandDoesNotExist,
    ];

    /// The rule's name on break lines and in the totals.
    fn label(self) -> &'static str {
        match self {
            BreakRule::MissingTarget => "missing target",
            BreakRule::EscapesRepository => "escapes repository",
            BreakRule::UndefinedReference => "undefined reference",
            BreakRule::MissingAnchor => "missing anchor",
            BreakRule::LineAnchorOutOfRange => "line anchor out of range",
            BreakRule::BacktickedPathNamesNothing => "backticked path names nothing",
            BreakRule::CitedCommandDoesNotExist => "cited command does not exist",
        }
    }
}

/// One way a judged document breaks a rule, at one line.
#[derive(Debug, PartialEq, Eq)]
struct Break {
    document: String,
    line: usize,
    rule: BreakRule,
    message: String,
}

impl Break {
    /// The break as it prints: `path:line: rule: message`.
    fn render(&self) -> String {
        format!(
            "{}:{}: {}: {}",
            self.document,
            self.line,
            self.rule.label(),
            self.message
        )
    }
}

/// What the rules found over a run: breaks, and verdicts for checks that could not run.
#[derive(Debug, Default)]
struct RuleFindings {
    breaks: Vec<Break>,
    not_run: Vec<Verdict>,
}

impl RuleFindings {
    /// Record a break of `rule` at `line` of `document`.
    fn broke(&mut self, document: &str, line: usize, rule: BreakRule, message: String) {
        self.breaks.push(Break {
            document: document.to_string(),
            line,
            rule,
            message,
        });
    }

    /// Record a check that could not run; it prints after the document verdicts.
    fn did_not_run(&mut self, verdict: Verdict) {
        self.not_run.push(verdict);
    }
}

/// One judged document as every rule sees it.
struct JudgedDocument<'a> {
    /// The document's repository-relative path.
    path: &'a str,
    scan: &'a ScannedDocument,
}

/// What every rule may consult beyond the document.
struct RuleContext<'a> {
    repo_root: &'a Path,
    tree: &'a TrackedTree,
}

/// A rule of the link check, judging one scanned document at a time.
trait DocumentRule {
    /// Whether the rule judges documents of `area`; frozen records take only the link rules.
    fn judges(&self, area: DocumentArea) -> bool;

    /// Judge one document, recording its breaks.
    fn judge(
        &mut self,
        document: &JudgedDocument<'_>,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    );

    /// Settle what the rule batched across the run, once every document is judged.
    fn finish(&mut self, _context: &RuleContext<'_>, _findings: &mut RuleFindings) {}

    /// The rule's own lines for the totals.
    fn totals(&self) -> Vec<String> {
        Vec::new()
    }
}

/// `cargo gates link-check`: judge the files `request` lists at `repo_root` over its
/// `--path` scope, print the verdicts, and return the exit status (0 held, 1 break, 2 did not
/// run).
pub(crate) fn verify_link_check(
    repo_root: &Path,
    request: &GateRequest,
    listing: BreakListing,
) -> u8 {
    let ignore_rules = GitIgnoreRules::new(repo_root);
    let commands = gates_command_tree();
    let mut rules: Vec<Box<dyn DocumentRule + '_>> = vec![
        Box::new(LinkTargets::new()),
        Box::new(BacktickedPaths::new(&ignore_rules)),
        Box::new(CommandCitations::new(&commands)),
    ];
    judge(
        repo_root,
        TrackedTree::load(repo_root, request.untracked),
        request,
        listing,
        &mut rules,
    )
    .print()
}

/// The gate over an already-attempted listing and a given rule list, so a failed listing and
/// a fixture tree take the same path as a real run.
fn judge(
    repo_root: &Path,
    listing: Result<TrackedTree, NotRun>,
    request: &GateRequest,
    break_listing: BreakListing,
    rules: &mut [Box<dyn DocumentRule + '_>],
) -> GateRun {
    let (tree, scope) = match prepare(GATE, Kind::Ban, repo_root, listing, request) {
        Ok(prepared) => prepared,
        Err(stopped) => return stopped,
    };
    let mut run = GateRun::new(
        GATE,
        request.untracked,
        vec![
            format!(
                "==> {GATE}: every link in the judged Markdown reaches a tracked file or folder, \
                 or a heading or line anchor; in the live documents every backticked repository path names something and every cited \
                 `cargo gates` command exists"
            ),
            scope_line(&scope, &tree),
        ],
    );
    let documents: Vec<(&str, DocumentArea)> = tree
        .files()
        .filter(|path| scope.contains(path))
        .filter_map(|path| judged_area(path).map(|area| (path, area)))
        .collect();
    if documents.is_empty() {
        run.verdicts
            .push(judged_nothing(GATE, Kind::Ban, repo_root, &scope));
        return run;
    }
    let context = RuleContext {
        repo_root,
        tree: &tree,
    };
    let mut findings = RuleFindings::default();
    let mut unread: BTreeMap<&str, Verdict> = BTreeMap::new();
    for (path, area) in &documents {
        match read_tracked(repo_root, path) {
            Err(cause) => {
                let verdict =
                    Verdict::did_not_run(format!("{path} could not be read"), Kind::Ban, cause);
                unread.insert(path, verdict);
            }
            Ok(text) => {
                let scanned = scan(&text);
                let document = JudgedDocument {
                    path,
                    scan: &scanned,
                };
                for rule in rules.iter_mut().filter(|rule| rule.judges(*area)) {
                    rule.judge(&document, &context, &mut findings);
                }
            }
        }
    }
    for rule in rules.iter_mut() {
        rule.finish(&context, &mut findings);
    }
    let RuleFindings { breaks, not_run } = findings;
    let mut by_document: BTreeMap<&str, Vec<&Break>> = BTreeMap::new();
    for found in &breaks {
        by_document.entry(&found.document).or_default().push(found);
    }
    let mut budget = match break_listing {
        BreakListing::First => FIRST_BREAKS,
        BreakListing::Every => usize::MAX,
    };
    for (path, _) in &documents {
        let verdict = match (unread.remove(path), by_document.get_mut(path)) {
            (Some(verdict), _) => verdict,
            (None, None) => Verdict::Held,
            (None, Some(found)) => {
                found.sort_by_key(|found| found.line);
                document_verdict(path, found, &mut budget)
            }
        };
        run.verdicts.push(verdict);
    }
    let unreadable = run
        .verdicts
        .iter()
        .filter(|verdict| matches!(verdict, Verdict::DidNotRun(..)))
        .count();
    run.verdicts.extend(not_run);
    run.totals = totals(&documents, &breaks, unreadable, rules, break_listing);
    run
}

/// The failed verdict of a document with breaks: a headline naming the count, and the breaks
/// themselves while `budget` lasts.
fn document_verdict(path: &str, breaks: &[&Break], budget: &mut usize) -> Verdict {
    let shown = breaks.len().min(*budget);
    *budget -= shown;
    let mut detail: Vec<String> = breaks[..shown].iter().map(|found| found.render()).collect();
    if shown > 0 && shown < breaks.len() {
        detail.push(format!("… {} more in this document", breaks.len() - shown));
    }
    Verdict::Failed(Finding {
        headline: format!("{path}: {} break(s)", breaks.len()),
        detail,
    })
}

/// The totals: documents judged, each rule's own lines, and the breaks by rule and by area.
fn totals(
    documents: &[(&str, DocumentArea)],
    breaks: &[Break],
    unreadable: usize,
    rules: &[Box<dyn DocumentRule + '_>],
    break_listing: BreakListing,
) -> Vec<String> {
    let frozen = documents
        .iter()
        .filter(|(_, area)| area.is_frozen())
        .count();
    let area_of: BTreeMap<&str, DocumentArea> = documents.iter().copied().collect();
    let failing: BTreeMap<&str, DocumentArea> = breaks
        .iter()
        .filter_map(|found| area_of.get_key_value(found.document.as_str()))
        .map(|(path, area)| (*path, *area))
        .collect();
    let mut lines = vec![format!(
        "  documents: {} judged ({frozen} frozen record(s)), {} with breaks, {unreadable} \
         unreadable",
        documents.len(),
        failing.len()
    )];
    lines.extend(rules.iter().flat_map(|rule| rule.totals()));
    lines.push(format!("  breaks by rule: {} in all", breaks.len()));
    for rule in BreakRule::ALL {
        let count = breaks.iter().filter(|found| found.rule == rule).count();
        lines.push(format!("    {}: {count}", rule.label()));
    }
    lines.push("  breaks by area:".to_string());
    for area in DocumentArea::ALL {
        let judged = documents.iter().filter(|(_, found)| *found == area).count();
        let broken = breaks
            .iter()
            .filter(|found| area_of.get(found.document.as_str()) == Some(&area))
            .count();
        let with_breaks = failing.values().filter(|found| **found == area).count();
        lines.push(format!(
            "    {}: {broken} break(s) in {with_breaks} of {judged} document(s)",
            area.label()
        ));
    }
    if break_listing == BreakListing::First && breaks.len() > FIRST_BREAKS {
        lines.push(format!(
            "  the first {FIRST_BREAKS} breaks are shown; --report lists all {}",
            breaks.len()
        ));
    }
    lines
}

#[cfg(test)]
#[path = "tests/link_check.rs"]
mod tests;

//! The link rule: every destination in a judged document reaches what it names.
//!
//! **Role:** judges each link a document's scan found. A checkout path must resolve to a tracked
//! file or a folder holding one without climbing above the repository root; a fragment must
//! match a heading or explicit anchor of a rendered Markdown target, or fit a `#L<n>[-L<m>]` line
//! anchor into any other file; a reference must be defined. External destinations are
//! counted, never fetched.
//!
//! **Position:** a [`DocumentRule`] of the link-check pipeline in [`super::super::link_check`];
//! it reads [`super::target_resolution`] for classification, resolution and fragments, and
//! [`super::heading_anchors`] for anchors.
//!
//! **Signals and state:** the anchors and line counts of every target read so far, keyed by path; all of it lives for one run.
//!
//! **Invariants:** a target that cannot be read is one "did not run" verdict, never a pass or a
//! break; every break
//! names the destination as written.

use std::collections::{BTreeSet, HashMap};

use verification_core::{Kind, Verdict};

use super::heading_anchors::document_anchors;
use super::judged_documents::DocumentArea;
use super::markdown_scan::{ScannedLink, scan};
use super::target_resolution::{
    Destination, FragmentNeed, Resolution, classify, decode_fragment, fragment_need,
    line_anchor_problem, line_count, resolve, unmatchable_anchor_problem,
};
use super::{BreakRule, DocumentRule, JudgedDocument, RuleContext, RuleFindings};
use crate::gate_run::read_tracked;

/// A checkout destination resolved, with what its query and fragment ask of the target.
#[derive(Debug)]
struct CheckoutTarget {
    resolution: Resolution,
    plain_view: bool,
    fragment: Option<String>,
}

/// How many destinations of each kind the rule judged.
#[derive(Debug, Default)]
struct LinkCounts {
    checkout: usize,
    external: usize,
}

/// The link rule's state for one run.
pub(super) struct LinkTargets {
    /// The anchors of every Markdown target read so far; `None` when it could not be read.
    anchors: HashMap<String, Option<BTreeSet<String>>>,
    /// The line count of every line-anchored target read so far; `None` when it could not be read.
    line_counts: HashMap<String, Option<usize>>,
    counts: LinkCounts,
}

impl LinkTargets {
    /// A rule with nothing read yet.
    pub(super) fn new() -> LinkTargets {
        LinkTargets {
            anchors: HashMap::new(),
            line_counts: HashMap::new(),
            counts: LinkCounts::default(),
        }
    }

    fn judge_link(
        &mut self,
        document: &str,
        link: &ScannedLink,
        own_anchors: &BTreeSet<String>,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) {
        let written = &link.destination;
        let line = link.line;
        match classify(written) {
            Destination::External => self.counts.external += 1,
            Destination::SameDocument { fragment } => {
                self.counts.checkout += 1;
                let anchor = decode_fragment(&fragment);
                if !anchor.is_empty() && !own_anchors.contains(&anchor) {
                    findings.broke(
                        document,
                        line,
                        BreakRule::MissingAnchor,
                        format!("`{written}`: this document has no heading or anchor `{anchor}`"),
                    );
                }
            }
            Destination::CheckoutPath {
                path,
                plain_view,
                fragment,
            } => {
                self.counts.checkout += 1;
                let target = CheckoutTarget {
                    resolution: if path.is_empty() {
                        Resolution::File(document.to_string())
                    } else {
                        resolve(document, &path, context.tree)
                    },
                    plain_view,
                    fragment,
                };
                if let Some((rule, message)) =
                    self.checkout_break(written, target, context, findings)
                {
                    findings.broke(document, line, rule, message);
                }
            }
        }
    }

    /// The break a checkout destination makes, if any.
    fn checkout_break(
        &mut self,
        written: &str,
        target: CheckoutTarget,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) -> Option<(BreakRule, String)> {
        let broken = match (target.resolution, target.fragment) {
            (Resolution::EscapesRepository, _) => (
                BreakRule::EscapesRepository,
                format!("`{written}` climbs above the repository root"),
            ),
            (Resolution::Missing(normalised), _) => (
                BreakRule::MissingTarget,
                format!(
                    "`{written}` resolves to `{normalised}`, which is no tracked file or folder"
                ),
            ),
            (Resolution::Folder(folder), Some(_)) => (
                BreakRule::MissingAnchor,
                format!("`{written}`: `{folder}` is a folder, which has no anchors"),
            ),
            (Resolution::File(file), Some(fragment)) => {
                match fragment_need(&file, target.plain_view, &fragment) {
                    FragmentNeed::Anchor(anchor) => {
                        let anchors = self.markdown_anchors(&file, context, findings)?;
                        if anchors.contains(&anchor) {
                            return None;
                        }
                        (
                            BreakRule::MissingAnchor,
                            format!("`{written}`: `{file}` has no heading or anchor `{anchor}`"),
                        )
                    }
                    FragmentNeed::Lines(first, last) => {
                        let lines = self.line_count(&file, context, findings)?;
                        let problem = line_anchor_problem(first, last, lines)?;
                        (
                            BreakRule::LineAnchorOutOfRange,
                            format!("`{written}`: {problem}"),
                        )
                    }
                    FragmentNeed::Unmatchable(anchor) => (
                        BreakRule::MissingAnchor,
                        format!(
                            "`{written}`: {}",
                            unmatchable_anchor_problem(&file, &anchor)
                        ),
                    ),
                }
            }
            (Resolution::File(_) | Resolution::Folder(_), None) => return None,
        };
        Some(broken)
    }

    /// The anchors of a tracked Markdown file, read once; `None`, reported once as "did not run",
    /// when the file cannot be read.
    fn markdown_anchors(
        &mut self,
        file: &str,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) -> Option<&BTreeSet<String>> {
        if !self.anchors.contains_key(file) {
            let anchors = read_target(file, context, findings, "its anchors")
                .map(|text| document_anchors(&scan(&text)));
            self.anchors.insert(file.to_string(), anchors);
        }
        self.anchors.get(file).and_then(Option::as_ref)
    }

    /// The line count of a tracked file, read once; `None`, reported once as "did not run", when
    /// the file cannot be read.
    fn line_count(
        &mut self,
        file: &str,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) -> Option<usize> {
        if !self.line_counts.contains_key(file) {
            let lines = read_target(file, context, findings, "its line anchors")
                .map(|text| line_count(&text));
            self.line_counts.insert(file.to_string(), lines);
        }
        self.line_counts.get(file).copied().flatten()
    }
}

impl DocumentRule for LinkTargets {
    fn judges(&self, _area: DocumentArea) -> bool {
        true
    }

    fn judge(
        &mut self,
        document: &JudgedDocument<'_>,
        context: &RuleContext<'_>,
        findings: &mut RuleFindings,
    ) {
        let own_anchors = document_anchors(document.scan);
        for reference in &document.scan.undefined_references {
            findings.broke(
                document.path,
                reference.line,
                BreakRule::UndefinedReference,
                format!(
                    "`[{}]` names no reference definition in this document",
                    reference.label
                ),
            );
        }
        for link in &document.scan.links {
            self.judge_link(document.path, link, &own_anchors, context, findings);
        }
        self.anchors
            .insert(document.path.to_string(), Some(own_anchors));
    }

    fn totals(&self) -> Vec<String> {
        let counts = &self.counts;
        let judged = counts.checkout + counts.external;
        vec![format!(
            "  links: {judged} judged — {} into this checkout, {} external and not fetched",
            counts.checkout, counts.external
        )]
    }
}

/// A tracked target's text, or `None` after recording why it could not be read.
fn read_target(
    file: &str,
    context: &RuleContext<'_>,
    findings: &mut RuleFindings,
    purpose: &str,
) -> Option<String> {
    match read_tracked(context.repo_root, file) {
        Ok(text) => Some(text),
        Err(cause) => {
            let what = format!("{file} could not be read to judge {purpose}");
            findings.did_not_run(Verdict::did_not_run(what, Kind::Ban, cause));
            None
        }
    }
}

#[cfg(test)]
#[path = "tests/link_targets.rs"]
mod tests;

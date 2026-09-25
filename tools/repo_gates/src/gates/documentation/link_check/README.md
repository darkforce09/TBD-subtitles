# Link check parts

The Markdown scan and the three rules behind the `link-check` gate: every link reaches what it
names, every backticked repository path in a live document names something in the checkout, and
every `cargo gates` command a live document cites exists.

## Contents

```text
tools/repo_gates/src/gates/documentation/link_check/
├── backticked_paths.rs   the rule for repository paths written as inline code in live documents
├── command_citations.rs  the rule for cited `cargo gates` commands, walked down the clap command tree
├── git_ignore_rules.rs   asks `git check-ignore` in one batch which unfound paths git ignores
├── heading_anchors.rs    the anchors a rendered document offers: heading slugs and `<a id|name>`
├── inline_html.rs        where an inline HTML tag ends, and the anchors an `<a>` tag declares
├── judged_documents.rs   which listed files the gate judges, and the area each belongs to
├── link_destination.rs   reads link destinations, titles, reference definitions and labels
├── link_targets.rs       the link rule: checkout paths, fragments, line anchors, references
├── markdown_inlines.rs   the inline pass: links, images, autolinks, code spans, anchors, heading text
├── markdown_lines.rs     the line shapes the block pass needs: list items, headings, quotes, indents
├── markdown_scan.rs      the block pass: reads a document once, as a renderer would
├── target_resolution.rs  classifies a destination and resolves it against the listed tree
└── tests/                unit tests, one file per part
```

## How it works

The gate in `tools/repo_gates/src/gates/documentation/link_check.rs` picks the judged documents,
scans each one once, and hands the scan to every rule that judges the document's area:

```text
listed files ──> judged_documents ──> each document: markdown_scan::scan
                 (area per file)          │  block pass (markdown_lines, fences)
                                          │  inline pass (markdown_inlines, link_destination, inline_html)
                                          ▼
                                   ScannedDocument: links, headings, code spans, fenced blocks
                                          │
            ┌─────────────────────────────┼──────────────────────────────┐
            ▼                             ▼                              ▼
      link_targets                 backticked_paths               command_citations
   (target_resolution,        (live documents only; unfound    (live documents only; walks
    heading_anchors)           paths batched to git_ignore_rules) crate::cli::Cli's command tree)
            └───────────── breaks, as `path:line: rule: message` ─────────────┘
```

**Judged documents.** Every Markdown file under the documentation root, every README.md anywhere,
and `CLAUDE.md`. Each lands in one area: live documentation, frozen records (a document other than
a README.md inside a frozen folder), the project instructions, or READMEs elsewhere. Frozen
records are judged only by the link rule.

**The scan.** The block pass sets aside what never renders as a link (front matter, fenced and
indented code blocks, HTML comments), groups the rest into paragraph and heading runs, and reads
the reference definitions; the inline pass finds links, images, autolinks, undefined references,
code spans and explicit anchors in each run, following CommonMark's precedence rules. Line numbers
are 1-based.

**The link rule.** A destination with a URI scheme or starting `//` is external: counted, never
fetched. A `#fragment` alone names the linking document. Any other destination is a checkout path,
resolved from the repository root when it starts with `/` and from the document's folder
otherwise, percent-decoded and normalised; it must name a listed file or a folder holding one
without climbing above the root. A fragment must match a heading anchor (derived as GitHub does)
or an `<a id|name>` of a rendered Markdown target, or be a `#L<n>` or `#L<n>-L<m>` line anchor
that fits any other file (or a Markdown file viewed with `?plain=1`). A full or collapsed
reference must be defined.

**The backticked-path rule.** An inline code span in a live document whose first segment is a
listed top-level folder is read as a repository path, with a `:N` line suffix and a `#fragment`
stripped; a trailing `/` asks for a folder. A span that is a glob, placeholder, set, variable,
command, URL or elision is counted and skipped; fenced code is not read. A path the listed tree
lacks is asked of `git check-ignore --stdin -z` in one batch when the run ends, and passes when
git ignores it (runtime output such as `target/`).

**The command-citation rule.** Every `cargo gates` in a live document's inline code spans and in
each line of its fenced blocks (a line ending in `\` continues) is walked down the command tree
of `crate::cli::Cli`: flags are skipped, a placeholder ends the walk, and the first positional
argument must be one of the gate names clap declares. A break names the command up to the first
word that names nothing.

Without `--report` the gate prints every failing document with its break count and the first 20
breaks in full; with it, every break. The totals count documents, links by kind, paths and
citations by outcome, breaks by rule and breaks by area.

## Boundaries

- Depends on: `tools/repo_gates/src/gate_run/` (the tree, `read_tracked`, `markdown_fences`,
  `path_regions`); `tools/repo_gates/src/layout.rs`; `tools/repo_gates/src/cli.rs` through
  `clap::CommandFactory`; `verification_core` (`Verdict`, `proc::Run`); the `git` program
  (`check-ignore`).
- Used by: `tools/repo_gates/src/gates/documentation/link_check.rs` only.
- Rules:
  - external destinations are never fetched (`external_links_are_counted_and_never_fetched`);
  - a target that cannot be read and a failed ignore batch are "did not run", never a pass or a
    break (`an_unreadable_target_did_not_run_once`, `a_failed_batch_did_not_run_and_breaks_nothing`,
    `a_checkout_git_cannot_read_did_not_run`);
  - frozen records are judged by the link rule alone
    (`frozen_records_are_never_judged_by_the_rule` in both rule test files);
  - nothing inside code, comments or front matter is a link or a heading
    (`code_spans_fences_and_indented_code_hold_no_links`, `html_comments_and_front_matter_hold_no_links`,
    `setext_headings_count_and_fenced_ones_do_not`);
  - the cited-command rule accepts exactly the runner's own gates
    (`the_gates_tree_resolves_its_own_commands`).

## Related documentation

- [README standard](/documentation/standards/readme_standard.md#writing-rules) — the link and
  path rules every README follows.
- [Documentation standards](/documentation/standards/documentation_standards.md#writing) — how
  documents link to each other.

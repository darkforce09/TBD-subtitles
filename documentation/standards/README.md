**Status:** live

# Standards

The rules for documents, READMEs, code and commits, and the templates that put them into
practice. They follow the TBD-Reforger standards, sized for a small single-binary app.

## Contents

```text
documentation/standards/
├── coding_standards.md          Rust rules: language ban, layering, file size, tests, comments, errors
├── commit_conventions.md        commit messages, what goes in one commit, the checks before it
├── documentation_standards.md   where documents live, names, status lines, links, lifecycle
├── readme_standard.md           the README core, the Contents grammar, folder kinds, writing rules
└── templates/                   one skeleton per README kind and document type, with worked samples
```

## How it works

[CLAUDE.md](/CLAUDE.md) states the laws in one line each; these documents give the detail, and the
templates turn them into skeletons to copy. Every rule a program can check names the
`cargo gates` gate that holds it; the rest are held in review.

## Code

- [Repository gates](/tools/repo_gates/) — the `cargo gates` checks behind these standards.
- [App architecture tests](/apps/tbd_subtitles/src/tests/) — the feature-folder rules of the app.

## Boundaries

- Depends on: the laws in CLAUDE.md and the decision log.
- Used by: every README, document, source file and commit in the repository.
- Rules: a rule marked as gated names its gate, and the gate exists in `tools/repo_gates/`
  (`cargo gates link-check` checks every cited `cargo gates` command).

## Related documentation

- [Documentation map](/documentation/README.md) — where each kind of document lives.

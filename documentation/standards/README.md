**Status:** live

# Standards

The rules for documents, code and commits. They are a trimmed version of the TBD-Reforger
standards, sized for a small single-binary app.

## Contents

```text
standards/
├── README.md                    this index
├── documentation_standards.md   document layout, names, status lines, READMEs, links, lifecycle
├── coding_standards.md          Rust rules: language ban, layering, file size, tests, comments, errors
└── commit_conventions.md        commit messages, what goes in one commit, the checks before it
```

## How it works

[CLAUDE.md](/CLAUDE.md) states the laws in one line each; these documents give the detail. Rules a
program can check are enforced by `tools/repo_gates` once milestone M0 builds it; each rule below
says whether a gate holds it.

## Related documentation

- [Documentation map](/documentation/README.md) — where each kind of document lives.

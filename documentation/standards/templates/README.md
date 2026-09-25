**Status:** live

# Documentation templates

Copyable skeletons for every README kind of the
[README standard](/documentation/standards/readme_standard.md) and for the document types under
`documentation/`: feature documents, runbooks, decision entries, glossary entries, known bugs and
research snapshots. Each template carries a worked sample written from this repository.

## Contents

```text
documentation/standards/templates/
├── decisions_entry.md              template for a decision log entry; sample: native runtimes allowed
├── feature_doc.md                  template for a feature document; sample: the desktop GUI
├── glossary_entry.md               template for a glossary entry; sample: worker process
├── known_bug.md                    template for a known bug; sample: the container's older FFmpeg
├── readme_area_root.md             README template for an area root; sample: the library crates
├── readme_command_line.md          README template for a command-line folder; sample: the app's CLI
├── readme_crate_root.md            README template for a crate root; sample: the child process crate
├── readme_data.md                  README template for a data folder; sample: an illustrative manifest
├── readme_documentation_folder.md  README template for a documentation folder; sample: the runbooks
├── readme_domain.md                README template for a domain or subsystem; sample: the job queue
├── readme_leaf.md                  README template for a leaf folder; sample: the stage names
├── research_snapshot.md            template for a frozen research snapshot; sample: the Rust ML stack
└── runbook.md                      template for a runbook; sample: the development environment
```

## How it works

Each template opens with its status line, its title and a **When to use** line naming the folders
or documents it fits. Then comes the skeleton: the sections in order inside a fenced `markdown`
block, every placeholder written as `<…>` and saying what goes there. A worked sample follows in a
second fenced block, written from a real folder or document of this repository and checked against
the code: a README sample's Contents block lists exactly its folder's children, and a document
sample quotes only commands, files and entries that exist. The one exception is the data sample,
which describes a folder the repository does not hold yet and says so.

The README templates are named `readme_<kind>.md` after the kind table in the README standard, and
each adds the kind sections that table lists. The document templates follow the section orders the
[documentation standards](/documentation/standards/documentation_standards.md) fix:

| Template | Sections, in order |
|---|---|
| `feature_doc.md` | Where it lives, Behaviour, Data, Design, Open work, Decisions |
| `runbook.md` | Prerequisites, numbered Steps with an **Expected:** line each, Verify, Troubleshooting, Related documentation |
| `decisions_entry.md` | a dated `###` heading, then Context, Decision, Consequences, Supersedes |
| `glossary_entry.md` | a `###` heading, the definition, In code, See |
| `known_bug.md` | Status, Symptom, Cause, Workaround, Fix, Related |
| `research_snapshot.md` | one numbered section per capability with its options and a Recommendation, then Recommended stack, Hard gaps, Sources |

The skeletons and samples sit in fences, so no gate reads them as documents: `readme-coverage` and
`readme-sections` judge only files named README.md, and `link-check` reads neither the links nor the
backticked paths inside a fence. It does check every `cargo gates` command a fence cites, so a
sample names only gates that exist. A writer picks the kind or the document type, copies the
skeleton, fills every placeholder from the code, and runs the gates the standard names.

## Code

- [Repository gates](/tools/repo_gates/) — the `readme-coverage`, `readme-sections`,
  `status-lines`, `markdown-placement`, `link-check` and `prose-rules` gates that check every
  README and document these templates shape.

## Boundaries

- Depends on: the README standard, which defines the README core, the kinds and their sections,
  and the documentation standards, which fix the document types' section orders, status lines and
  lifecycle.
- Used by: everyone who writes or reviews a README in the code trees or a document under
  `documentation/`; the README standard and the READMEs of the features, research and runbooks
  folders link the template their documents follow.
- Rules: one template per README kind, named as the kind table names it, and one per document
  type; each holds a status line, a **When to use** line, a skeleton and one worked sample, both
  fenced; a README sample's Contents block matches its folder; every `cargo gates` command a
  template cites exists, fences included (`cargo gates link-check`); no template passes 500 lines
  (`cargo gates markdown-placement`).

## Related documentation

- [README standard](/documentation/standards/readme_standard.md) — the README core, the Contents
  grammar, the kinds and the gates.
- [Documentation standards](/documentation/standards/documentation_standards.md) — where documents
  live, their status lines, section orders and lifecycle.

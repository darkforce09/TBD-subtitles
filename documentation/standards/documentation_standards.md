**Status:** live

# Documentation standards

How documents in this repository are laid out, named, written and kept current. Rules held by a
program name the `cargo gates` gate that holds them; the rest are held in review.

## Where documents live

- Every document lives under `documentation/`. Code folders hold only their README.md
  (`cargo gates markdown-placement`).
- A document about one part of the code sits at the code's path with `apps/`, `crates/` and
  `src/` left out: the app's feature folder `apps/tbd_subtitles/src/job_queue/` is documented in
  a `tbd_subtitles/job_queue/` folder under `documentation/`, created with its first document.
- Topics that span the code have their own folders: `architecture/`, `optimizations/`,
  `research/`, `features/`, `runbooks/`, `standards/`, and the decision log in `decisions/`;
  `glossary.md`, `roadmap.md` and `vision_and_goals.md` sit at the top.

## Files

- Names are snake_case `.md`, apart from `README.md`.
- The first line of every document under `documentation/` is its status line:
  `**Status:** live`, `**Status:** frozen record (YYYY-MM-DD)` or `**Status:** archived`
  (`cargo gates status-lines`). Code READMEs carry no status line (`cargo gates
  readme-sections`).
- A live document stays within 500 lines; a longer one splits into a folder with a README index
  (`cargo gates markdown-placement`).
- Live documents carry no dates, except decision entries and the status line of frozen records.

## READMEs

Every folder in `documentation/` and in the code trees has a README.md
(`cargo gates readme-coverage`), written to the
[README standard](/documentation/standards/readme_standard.md): a title, a purpose, a Contents
block that lists the folder exactly, How it works, the sections of the folder's kind, Boundaries
with its three bullets, and Related documentation (`cargo gates readme-sections`). Folders named
`tests` or `generated`, and folders whose name begins with `.`, need none.

## Templates

Every README kind and document type has a skeleton with a worked sample in
[templates](/documentation/standards/templates/README.md): feature documents, runbooks, decision
entries, glossary entries, known bugs and research snapshots. A new document starts from its
template.

## Feature documents

Sections in order: Where it lives, Behaviour, Data, Design, Open work, Decisions. Planned
features say so in their first paragraph.

## Decision entries

In the decision log (`decisions/`, one file per subject, each within 500 lines), one `###`
heading per decision: `### YYYY-MM-DD — <the decision>`, then **Context**, **Decision**,
**Consequences**, **Supersedes**. A changed decision gets a new entry naming the old one; old
entries are never reworded.

## Runbooks

Prerequisites, then numbered steps (one command each, followed by an **Expected:** line), then how
to verify, Troubleshooting, and Related documentation.

## Writing

- Plain words for a reader who has not seen the code. Define a term in the
  [glossary](/documentation/glossary.md) and link its first use.
- Present tense in live documents and READMEs: no history words (`cargo gates prose-rules`).
- Links start at the repository root (`/documentation/...`), never `../`. Every link, anchor and
  backticked repository path in a live document resolves, and every cited `cargo gates` command
  exists (`cargo gates link-check`).
- Diagrams are ASCII inside `text` code blocks.
- Code READMEs never name tickets or milestones; the roadmap does (`cargo gates prose-rules`).

## Lifecycle

- **Live** documents track the code and change in the same commit as the code they describe.
- **Frozen records** (research snapshots in `research/`) keep their words; only broken links are
  fixed. New facts go into a new snapshot.
- **Archived** documents record history and are never current.
- Authority, highest first: running code, CLAUDE.md, decisions, the documentation README and these
  standards, live documents, frozen records.

## Related documentation

- [README standard](/documentation/standards/readme_standard.md) — the README core, Contents
  grammar and folder kinds.
- [Templates](/documentation/standards/templates/README.md) — skeletons for every README kind and
  document type.

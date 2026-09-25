**Status:** live

# Documentation standards

How documents in this repository are laid out, named, written and kept current.

## Where documents live

- Every document lives under `documentation/`. Code folders hold only their README.md.
- A document about code sits at the code's path with `apps/`, `crates/` and `src/` left out:
  `apps/tbd_subtitles/src/job_queue/` → `documentation/tbd_subtitles/job_queue/`.
- Topics that span the code have their own folders: `architecture/`, `research/`, `features/`,
  `runbooks/`, `standards/`; `glossary.md`, `decisions.md`, `roadmap.md` and
  `vision_and_goals.md` sit at the top.

## Files

- Names are snake_case `.md`, apart from `README.md`.
- The first line is the status line: `**Status:** live`, `**Status:** frozen record (YYYY-MM-DD)`
  or `**Status:** archived`. *(gate)*
- A live document stays within 500 lines; split a longer one into a folder with a README index.
  *(gate)*
- Live documents carry no dates, except decision entries and the status line of frozen records.

## READMEs

Every folder in `documentation/`, and every code folder once code exists, has a README.md
*(gate)*, with these sections in order:

1. `# Title` and one to three sentences on what the folder holds.
2. `## Contents` — a `text` code block listing the direct children with a short note each.
3. `## How it works` — how the parts fit together.
4. For code folders: `## Boundaries` with exactly three bullets — Depends on, Used by, Rules (each
   rule names the test or gate that holds it).
5. `## Related documentation` — links to the deeper documents.

Test folders, generated folders and dot-folders need no README.

## Feature documents

Sections in order: Where it lives, Behaviour, Data, Design, Open work, Decisions. Planned
features say so in their first paragraph.

## Decision entries

In `decisions.md`, one `###` heading per decision: `### YYYY-MM-DD — <the decision>`, then
**Context**, **Decision**, **Consequences**, **Supersedes**. A changed decision gets a new entry
naming the old one; old entries are never reworded.

## Runbooks

Prerequisites, then numbered steps (one command each, followed by an **Expected:** line), then how
to verify, Troubleshooting, and Related documentation.

## Writing

- Plain words for a reader who has not seen the code. Define a term in the
  [glossary](/documentation/glossary.md) and link its first use.
- Links start at the repository root (`/documentation/...`), never `../`. Every link and every
  backticked repository path must resolve. *(gate)*
- Diagrams are ASCII inside `text` code blocks.
- READMEs never name tickets or milestones' internal progress; the roadmap does.

## Lifecycle

- **Live** documents track the code and change in the same commit as the code they describe.
- **Frozen records** (research snapshots) keep their words; only broken links are fixed. New facts
  go into a new snapshot.
- **Archived** documents record history and are never current.
- Authority, highest first: running code, CLAUDE.md, decisions, the documentation README and these
  standards, live documents, frozen records.

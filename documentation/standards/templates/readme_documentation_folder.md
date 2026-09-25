**Status:** live

# README template: documentation folder

**When to use:** any folder under `documentation/`, the root included: a topic folder such as
`documentation/runbooks/` or `documentation/research/`, a folder that mirrors a code folder, or a
long document split into a folder. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the documentation folder kind adds Code.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there. The status line
`**Status:** live` comes first, as it does for every README under `documentation/`, the index of a
folder of frozen records included.

````markdown
**Status:** live

# <What the documents cover, in plain words: no path, no backticks>

<One to three sentences: what the folder documents and who reads it.>

## Contents

```text
<repository path of the folder>/
├── <child folder>/   <what it covers: a lowercase phrase, no closing period>
├── <document>.md     <what it covers>
└── <document glob>   <the collection, when the documents are alike>
```

## How it works

<How the documents are organised: which template each follows, how they are named, which one to
read first, and how a new one is added. Leave the section out when the folder has no child folders
besides exempt ones and at most three files.>

## Code

- [<code folder name>](/<repository path of a code folder>/) — <what the documents say about it>

## Boundaries

- Depends on: <the templates and standards the documents follow, and the sources they draw on>
- Used by: <the documents, READMEs and code comments that link here, found with git grep>
- Rules: <the invariants a change must keep: naming, status lines, what may change and what is
  frozen, and the gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

Written from `documentation/runbooks/`, a folder of two runbooks. It keeps How it works, although
two files would let it leave the section out, because a new runbook's writer needs the rules it
states. The sample sits in a fenced block, so no gate reads it as a README; the folder's own
README.md is written from the same files and may differ.

````markdown
**Status:** live

# Runbooks

Step-by-step procedures: setting up and checking the development environment, and handing the
project to the next working session.

## Contents

```text
documentation/runbooks/
├── continue_in_claude_code.md   the prompt that starts the next session in the Claude Code GUI
└── development_environment.md   host and container, GPU, paths, toolchain, FFmpeg, and their checks
```

## How it works

Each runbook follows the [runbook template](/documentation/standards/templates/runbook.md): its
prerequisites, then numbered steps, each one command with an **Expected:** line, then how to
verify the result, what to do when it fails, and the related documents. A runbook is named in
snake_case after its procedure. Commands are copied as written; when one stops matching the code,
the runbook is fixed in the same commit as the change that broke it.

## Code

- [The app](/apps/tbd_subtitles/) — the binary the environment runbook builds and launches.
- [Repository gates](/tools/repo_gates/) — the `cargo gates` checks the runbooks cite.

## Boundaries

- Depends on: the host and container facts in [CLAUDE.md](/CLAUDE.md), and the runbook template.
- Used by: people and AI sessions setting up, building or handing over the project; CLAUDE.md and
  the app's README link the environment runbook.
- Rules: each step holds one command and its expected result; every cited `cargo gates` command
  exists (`cargo gates link-check`); each runbook stays within 500 lines
  (`cargo gates markdown-placement`).

## Related documentation

- [CLAUDE.md](/CLAUDE.md) — the environment rules in short.
- [Roadmap](/documentation/roadmap.md) — what the next session works on.
````

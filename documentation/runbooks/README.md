**Status:** live

# Runbooks

Step-by-step procedures: setting up and checking the development environment, and handing the
project to the next working session.

## Contents

```text
documentation/runbooks/
├── continue_in_claude_code.md   the prompt that starts the next session in the Claude Code GUI
└── development_environment.md   host and container, GPU, paths, toolchain, FFmpeg, build and check
```

## How it works

Each runbook follows the [runbook template](/documentation/standards/templates/runbook.md): its
prerequisites, then numbered steps, each one command with an **Expected:** line, then how to
verify the result and what to do when it fails. Commands are copied as written; when one stops
matching the code, the runbook is fixed in the same commit as the change that broke it.

## Code

- [The app](/apps/tbd_subtitles/) — the binary the environment runbook builds and launches.
- [Repository gates](/tools/repo_gates/) — the `cargo gates` checks the environment runbook runs.

## Boundaries

- Depends on: the host and container facts in [CLAUDE.md](/CLAUDE.md), and the runbook template.
- Used by: people and AI sessions setting up, building or handing over the project.
- Rules: every cited `cargo gates` command exists (`cargo gates link-check`); each step holds one
  command and its expected result.

## Related documentation

- [CLAUDE.md](/CLAUDE.md) — the environment rules in short.
- [Roadmap](/documentation/roadmap.md) — what the next session works on.

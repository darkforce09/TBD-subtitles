**Status:** live

# Runbooks

Step-by-step procedures: setting up and checking the development environment, and handing the
project to the next working session.

## Contents

```text
runbooks/
├── README.md                     this index
├── development_environment.md    host vs container, GPU, paths, toolchain, FFmpeg, CUDA libraries
└── continue_in_claude_code.md    the prompt that starts the next session in the Claude Code GUI
```

## How it works

Each runbook lists its prerequisites, then numbered steps, each a command with an **Expected:**
line, then how to verify the result and what to do when it fails. Commands are copied as written;
when one stops matching reality, fix the runbook in the same commit as the change that broke it.

## Related documentation

- [CLAUDE.md](/CLAUDE.md) — the environment rules in short.
- [Roadmap](/documentation/roadmap.md) — what the next session works on.

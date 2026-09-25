**Status:** live

# Features

One document per user-facing feature: what the user sees and does, the data it uses, its design,
its open work and the decisions behind it. A feature not yet built says so in its first paragraph.

## Contents

```text
documentation/features/
├── automation.md               watch folders, the Dolphin right-click entry, single-instance hand-off
├── gui.md                      the desktop window: job queue, progress, reports, review of flagged lines
└── japanese_onscreen_text.md   translated subtitles for Japanese text shown on screen
```

## How it works

Each document follows the [feature document template](/documentation/standards/templates/feature_doc.md):
Where it lives, Behaviour, Data, Design, Open work, Decisions. The subtitle pipeline itself is not
a feature document; it is the [pipeline](/documentation/architecture/pipeline.md).

## Code

- [The app](/apps/tbd_subtitles/) — the binary whose window and commands these features describe.

## Boundaries

- Depends on: the feature document template, the roadmap for open work, and the decision log.
- Used by: the roadmap, which links each feature from the milestone that builds it, and the
  READMEs of the app's feature folders.
- Rules: each document keeps the template's sections in order and stays within 500 lines
  (`cargo gates markdown-placement`).

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the milestone that builds each feature.
- [Vision and goals](/documentation/vision_and_goals.md) — why each feature exists.

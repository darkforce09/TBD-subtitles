**Status:** live

# Features

One document per user-facing feature: what the user sees and does, the data it uses, its design,
its open work and the decisions behind it. Until the code exists, each document describes the
planned behaviour and says so.

## Contents

```text
features/
├── README.md                   this index
├── gui.md                      the desktop window: job queue, progress, reports, review of flagged lines
├── automation.md               watch folders, the Dolphin right-click entry, single-instance hand-off
└── japanese_onscreen_text.md   translated subtitles for Japanese text shown on screen
```

## How it works

Each document follows the feature-document shape: Where it lives, Behaviour, Data, Design, Open
work, Decisions. The subtitle pipeline itself is not a feature document; it is the
[pipeline](/documentation/architecture/pipeline.md) in `architecture/`.

## Related documentation

- [Roadmap](/documentation/roadmap.md) — the milestone that builds each feature.
- [Vision and goals](/documentation/vision_and_goals.md) — why each feature exists.

**Status:** live

# README template: domain or subsystem

**When to use:** a folder with child folders of its own that is none of the more specific kinds:
a crate's `src/` root, one of the app's feature folders such as
`apps/tbd_subtitles/src/job_queue/`, or a group of modules such as a crate's backend folders. The
[README standard](/documentation/standards/readme_standard.md) defines every rule this template
follows; the domain kind adds Public surface.

## Skeleton

Copy the block and replace every `<…>` placeholder; each one says what goes there.

````markdown
# <Name of the domain or subsystem, in plain words: no path, no backticks>

<One to three sentences: what the domain or subsystem is responsible for.>

## Contents

```text
<repository path of the folder>/
├── <child folder>/  <what it is for: a lowercase phrase, no closing period>
├── <file>           <what it is for; entries run in case-insensitive name order>
└── mod.rs           <the module tree and what it re-exports>
```

## How it works

<How a call, a frame or a job moves through the children, the main types, and the invariants that
span them. Name each child's part in one clause; the child's own README holds the detail.>

## Public surface

- <module::item>: <what it is, and who outside the folder uses it>

## Boundaries

- Depends on: <the modules and crates the folder uses, read from its imports>
- Used by: <every user outside the folder, found with git grep>
- Rules: <the invariants a change here must keep, and the test or gate that checks each>

## Related documentation

- [<document title>](/documentation/<path to the document>) — <what it covers>
````

## Worked sample

Written from `apps/tbd_subtitles/src/job_queue/`, the application module that uses it and the
architecture tests in `apps/tbd_subtitles/src/tests/architecture_rules.rs`. The sample sits in a
fenced block, so no gate reads it as a README; the folder's own README.md is written from the same
code and may differ.

````markdown
# Job queue

The window's queue of videos waiting for subtitles: the panel on the left that lists them, and the
rules for adding and removing them.

## Contents

```text
apps/tbd_subtitles/src/job_queue/
├── events.rs  what the queue panel asks the application to do
├── mod.rs     the module tree of the feature
├── models/    the borrowed view of the queue the panel draws from
├── services/  adding videos to the queue and taking them out, with their unit tests
└── ui/        the queue panel
```

## How it works

The queue itself belongs to the application state; this feature never holds it. Each frame the
application lends the panel a `JobQueueView`, a read-only borrow of the queued videos in run order.
`ui::queue_panel_ui` draws one row per video, its file name with the full path on hover, and a
remove button; a click pushes `JobQueueEvent::Remove(index)` instead of changing anything. After
the frame the application turns each event into an action and applies it through
`services::queue_editing`: `add_videos` appends the videos not already queued and skips empty
paths, keeping the order given, and `remove_video` ignores an index past the end.

```text
application ──JobQueueView──▶ ui::queue_panel_ui ──JobQueueEvent──▶ application
     │                                                                   │
     └──────────────── services::queue_editing ◀───── Action ────────────┘
```

## Public surface

- `events::JobQueueEvent`: the one event, `Remove(index)`, which `application/events.rs` turns
  into `Action::RemoveFromQueue`.
- `models::view::JobQueueView`: the per-frame borrow `application/feature_views.rs` builds.
- `services::queue_editing::{add_videos, remove_video}`: the queue changes the application applies
  after each frame.
- `ui::queue_panel_ui`: the panel `application/feature_views.rs` draws in the left side panel.

## Boundaries

- Depends on: `eframe::egui` in `ui/` only; `crate::core::ui` for the muted text colour.
- Used by: the `application` module alone.
- Rules:
  - `models/` and `services/` never import egui or eframe; the feature never imports `application`
    or `cli`; no module but `application` imports this feature's `ui`
    (`dependency_boundaries_and_external_test_placement_are_enforced` in
    `apps/tbd_subtitles/src/tests/architecture_rules.rs`);
  - the feature keeps its `models/`, `services/` and `ui/` folders, each with a `mod.rs`
    (`module_roots_and_documentation_describe_the_entire_source_tree`, same file);
  - the panel changes no state while a frame is drawn: every change is an event the application
    applies after the frame.

## Related documentation

- [Desktop GUI](/documentation/features/gui.md) — the queue, progress and review the window is
  built to show.
````

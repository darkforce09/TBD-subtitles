# Text review state

Plain data for a loaded visual review and its asynchronous previews.

## Contents

```text
apps/tbd_subtitles/src/text_review/models/
└── mod.rs  Session, Picture, Comparison and Event
```

## How it works

A session keeps source observations, saved corrections and the current draft separately. Pictures contain bounded RGB buffers and serial numbers for texture reuse.

## Boundaries

- Depends on: `job_model` and `std`.
- Used by: text review services, UI and application actions.
- Rules: no rendering or filesystem operations.

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.

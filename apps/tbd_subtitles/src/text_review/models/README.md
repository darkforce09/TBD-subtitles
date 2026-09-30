# Text review state

Plain data for a loaded visual review and its asynchronous previews.

## Contents

```text
apps/tbd_subtitles/src/text_review/models/
├── localized.rs  LocalizedReview, PreviewMode, Replacement, MaskPlate, ReplacementPictures, Mask
└── mod.rs        Session, Picture, Comparison and Event
```

## How it works

A session keeps source observations, saved corrections and the current draft separately. Pictures contain bounded RGB buffers and serial numbers for texture reuse. For a job that writes a localized video, the session also holds `LocalizedReview`: the localized video and its subtitle file once written, each occurrence's replacement (drawn in, left out with the reason, or not decided yet) with its keyframe plate's erase mask and what the read-back check read, which picture the right preview shows (`PreviewMode::Subtitles` or `Localized`), whether the erase mask shows, and the selected occurrence's decoded plate and mask. A comparison's right picture is absent while it would show a localized video not written yet.

## Boundaries

- Depends on: `job_model` and `std`.
- Used by: text review services, UI and application actions.
- Rules: no rendering or filesystem operations.

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.

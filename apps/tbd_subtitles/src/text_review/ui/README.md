# Text review rendering

The Check Text list, editor and original/rendered comparison.

## Contents

```text
apps/tbd_subtitles/src/text_review/ui/
├── mod.rs          rendering exports
├── preview.rs      bounded textures and playback controls
├── replacement.rs  the localized video: mode control, replaced plate, erase mask, status
└── review.rs       occurrence list and correction editor
```

## How it works

The view borrows a session and decoded comparison, then returns events for application actions. Preview shows pixels from the actual ASS renderer. For a job that writes a localized video, a segmented control over the right picture switches between Subtitles and Localized video (the default); while the localized video is not written, the right picture is the selected occurrence's replaced plate with "Localized video not written yet" under it, or the reason it was not replaced. Show erase mask, beside the original picture's title, lays the keyframe plate's mask over the original picture in a translucent accent, on the plate's rectangle scaled to the picture. A line under the time says "Replaced in the video" in green or "Not replaced in the video: <reason>" in orange, with what the read-back check read in grey under it ("Checked: English reads back as …" or "Checked: Japanese still reads …"), and the treatment Replace reads "Replace in the video".

## Boundaries

- Depends on: text review models and core UI styling.
- Used by: application feature views.
- Rules: rendering does not start processes or write corrections.

## Related documentation

- [Check Text](/apps/tbd_subtitles/src/text_review/) — the feature.

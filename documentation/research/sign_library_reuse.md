**Status:** frozen record (2026-10-01)

# Sign library reuse over Dressrosa 11–15

How many approved signs the sign library of the
[binary storage plan](/documentation/architecture/binary_storage_plan.md) (phase 6) lets later
episodes reuse. Measured on 2026-10-01 on the host with the AppImage built from the phase 5 and
6 code.

## Method

The library was emptied, then the episodes ran in order: Dressrosa 11 rerun from `text_mask`
(its earlier documents stored once by a throwaway seeder), Dressrosa 12 to 15 from scratch. Each
job's step records give `text_translate`'s `library_matches`, `text_compose`'s `library_styles`
and `output`'s `library_added` and `library_joined` notes (`tbd-subtitles dump <video>
step_records <step>`). A match needs the same NFKC-normalised Japanese and a keyframe-crop dHash
within Hamming distance 6.

## Results

| Episode | Wall time | Occurrences detected | Translated | Library matches | Signs added | Library after |
|---|---|---|---|---|---|---|
| Dressrosa 11 (from `text_mask`) | 5.0 min | — | — | 0 | 7 | 1.1 MB |
| Dressrosa 12 | 14.2 min | 80 | 33 | 0 | 7 | 3.7 MB |
| Dressrosa 13 | 12.0 min | 37 | 6 | 0 | 0 | 3.7 MB |
| Dressrosa 14 | 14.9 min | 49 | 16 | 0 | 5 | 7.3 MB |
| Dressrosa 15 | 13.8 min | 43 | 18 | 0 | 4 | 7.3 MB |

## Findings

- No episode reused a sign: 23 signs were approved and recorded, and none recurred in a later
  episode, so no model call was saved on this batch.
- The approved signs of these episodes are one-off: name cards and scene writing, each shown in
  one episode. Recurring cards (an arc title, a location card shown every episode) are what the
  library can reuse; this batch had none that passed the read-back check.
- The library costs little: 7.3 MB for 23 signs, opened for one transaction per lookup or
  recording, and no step's time changed measurably.
- Whether reuse pays off needs a longer run of episodes or a looser match (the same text with a
  wider hash distance); this record does not change the matching rule.

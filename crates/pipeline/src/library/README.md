# Sign library

The library of approved signs shared by every episode: `library.redb` in the app's data folder
(`~/.local/share/tbd-subtitles/library.redb`, beside the default `work/` folder; one per user, not
one per work folder). A sign is an occurrence whose replacement `text_verify` kept and that no
owner correction rejected; a later occurrence of another job with the same writing starts from its
English and lettering style, and is still erased, composed and read back in its own frames.

## Contents

```text
crates/pipeline/src/library/
├── key.rs     the key: the Japanese in NFKC without whitespace, the crop's 64-bit difference hash
├── mod.rs     `Library`: lookup, record, remove, size and clear, one transaction each, with retry
├── signs.rs   a job's documents against the library: matches, their digest, styles, approvals
└── tests/     unit tests for keys, the library and its busy retry, matching and approvals
```

## How it works

One table, `signs`, keyed by `(normalised Japanese, crop hash)`, holds an rkyv archive of a
`job_model::onscreen::LibrarySign`: the Japanese, English, confidence, `LetteringStyle`, the patch
and mask of the approved replacement's first plate, the jobs that recorded it (the first is its
origin) and when it was added. A `meta` table holds `LAYOUT_VERSION`; a library at another version
loses its signs whole. The crop hash is a difference hash: the occurrence's first crop (its
keyframe crop) shrunk to 9 by 8 grey pixels, one bit per horizontal neighbour pair. A sign
matches an occurrence with the same normalised Japanese and a hash at most `MATCH_DISTANCE` (6)
bits away; the nearest wins.

```text
text_track document ──▶ signs::matches ──▶ text_translate: the sign's English, `library` provenance,
                        (other jobs only)     no Claude call for a keyframe of known signs alone
text_review document ─▶ signs::matches ──▶ text_compose: the sign's LetteringStyle for pending ones
text_verify + text_typeset ─▶ signs::approved ──▶ Library::record_all   (in the output step)
Check Text: a correction that changes or removes the English, keeps the Japanese, or a retry
  ──▶ signs::forget ──▶ Library::remove
```

No process keeps the file open, because a job run from a terminal and the window can run at the
same time and a second open fails at once (`DatabaseAlreadyOpen`). Every call opens it read-write
(which also repairs a file a killed writer left) for one transaction and closes it; while another
handle holds it the open is retried every 50 ms for up to 5 s, then the call fails. A call that
only reads or removes creates no file. Matching hashes every crop before it opens the library,
so a transaction holds the file only for its key lookups. The workers of `text_translate` and
`text_compose` open the library themselves: the runner names it in `LOCATION_VARIABLE`
(`TBD_SUBTITLES_LIBRARY`, empty for none), and `resume` computes the same matches for those two
steps' fingerprints. A job never matches a sign that came from itself, so the signs its own output
records never make its own steps stale; a recorded sign that is held already keeps its English,
style, patch and mask and only names the job, so a later run of another job keeps its
fingerprints. With an empty library, or none, no fingerprint, document or file changes.

## Public surface

- `Library` (`at`, `from_environment`, `lookup`, `lookup_all`, `record`, `record_all`, `remove`,
  `size`, `clear`), `LibrarySize`, `Recorded`, `default_path`, `reads_library`, `FILE_NAME`,
  `LOCATION_VARIABLE`.
- `key::{normalised, difference_hash, crop_hash, distance, MATCH_DISTANCE}`.
- `signs::{Matches, matches, digest, start_from_styles, approved, forget}`.

## Boundaries

- Depends on: `job_model::onscreen` (`LibrarySign`, the text and replacement documents), `redb`,
  `rkyv`, `image`, `unicode-normalization`, `serde_json`, `sha2` and `inference::model_store` for
  the data folder.
- Used by: `crate::resume`, `crate::runner`, `crate::tasks` (`onscreen`, `replace`, `layout`), and
  the window's Check Text and Settings (`apps/tbd_subtitles/src/text_review/`,
  `apps/tbd_subtitles/src/settings/`).
- Rules:
  - an identical crop is 0 bits away, a one-pixel change at most 2, another sign far beyond
    `MATCH_DISTANCE` (`tests/key.rs`);
  - a call waits for another handle to close and gives up after its wait
    (`a_call_waits_while_another_handle_holds_the_file_and_then_succeeds`,
    `a_call_gives_up_when_the_file_stays_held`);
  - a job matches only other jobs' signs, and an empty library matches nothing and hashes nothing
    (`occurrences_match_signs_of_other_jobs_by_text_and_crop`,
    `an_empty_library_matches_nothing_and_its_digest_is_none`);
  - only a baked replacement whose every reading passed is approved
    (`a_finished_job_approves_only_baked_replacements_that_read_back`).

## Related documentation

- [Binary storage plan](/documentation/architecture/binary_storage_plan.md#library-shared-by-episodes)
  — the library's place beside the job databases.
- [Video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md) — the
  steps that read and record signs.

**Status:** frozen record (2026-09-30)

# Localized video polish on Dressrosa 28

The owner reviewed the first localized videos in Check Text on 2026-09-30 and listed what fell
short of the Dressrosa 11 Rebecca card and 海 wall, the quality they asked for everywhere. This
record lists those issues, what changed, and what Dressrosa 28 and a Dressrosa 11 rerun showed
afterwards, with the issues still open. Every run below is `tbd-subtitles process <video>
--rerun text_review` from the AppImage on the Bazzite host: no Claude call, the audio and the
earlier visual steps resumed. Numbers come from each job's `visual/text_review.json`,
`visual/text_compose.json`, `visual/text_verify.json` and `visual/localized_video.json`; the
frames were rendered with the same FFmpeg filter chain Check Text uses. The steps are described
in the [video inpainting pipeline](/documentation/architecture/video_inpainting_pipeline.md).

## The owner's issues

| Time | Writing | Seen before the change |
|---|---|---|
| 2:37.3–2:38.0 | 錦えもんサイド name card | Replaced on one frame, the Japanese back on a later one, then garbled double lettering. |
| 3:07.6 | ワノ国の侍 / 夕立ち カン十郎 title card | Not replaced; only the furigana were separate occurrences. Dialogue sat over the card. |
| 5:55.5 | 王宮 / 4段目 / 3段目 / 2段目 / 1段目 level sign | Tiny English over the furigana alone while the kanji stayed Japanese. |
| 11:04.3 | 花畑 / 現在地 speech bubbles | Acceptable in the picture; black labels at the top repeated the English. |
| 12:55.3 | 幹部塔 / スクラップ場 / 錦えもんサイド | English drawn over Japanese that was not erased; dialogue over the card. |

## What changed

- **One occurrence per sign.** A rounding error split one sign at a one-frame gap, and Claude's
  own boxes duplicated the detector's. Occurrences of the same writing in the same place now
  join (the detector's box survives), and furigana fold into the line they annotate as `ruby`
  boxes, erased with it and never lettered alone.
- **Erasing.** The erase area takes in the furigana; a card whose box runs through its furigana
  and frame rule, dense outlined glyphs, and a static sign whose detector boxes jitter all
  separate now. Claude-found boxes are padded and refitted to the ink. A second replacement over
  the same sign is refused.
- **Approval by reading back.** The new `text_verify` step rebuilds sampled finished frames and
  has the local PP-OCRv5 detector and reader read them: a replacement stays in the video only if
  no Japanese is read where the writing was and its English reads back. No API call is made.
- **Localized subtitles.** `<video>.localized.ass` holds dialogue and sound cues only, and a line
  moves to the top while English lettered into the picture sits under it.

## Dressrosa 28

| | Before | After |
|---|---|---|
| Occurrences after review | 166 | 80 |
| Replacement candidates | 91 | 41 |
| Replacements in the video | 54 (duplicates of one sign counted apart) | 25, all approved by the read-back |
| Black labels in the localized subtitles | yes | none |
| `text_mask` / `text_inpaint` / `text_compose` / `text_verify` | — | 22 s / 47 s / 1 s / 38 s |
| `localized_video` | — | 230 s |

Fallbacks after the change: 8 could not be separated, 5 still show Japanese when read back, 1
moves in a way that could not be followed, 1 does not read back, 1 covered by another
replacement.

| Time | After |
|---|---|
| 2:37.3–2:37.7 | "Kin'emon's Side" on every frame; the bubbles bounce in with their Japanese for about half a second (000521 and 000524 read back with Japanese left and stay unreplaced). |
| 2:38.0 | "Executive Tower", "Scrap Yard", "Kin'emon's Side", one lettering each. |
| 3:07.6 | "Samurai of Wano Country" / "Yudachi Kanjuro" with the furigana erased; dialogue at the top. |
| 5:55.5–5:58.8 | "Royal Palace", "Level 4", "Level 3", "Level 1", "Colosseum Corps"; 2段目 stays Japanese. |
| 11:04.3 | "Flower Field", "Current Location"; no labels. |
| 12:55.3 | "Kin'emon Side" and "Scrap Yard"; 幹部塔 stays Japanese because its furigana かん とう read back unerased; dialogue at the top. |

## Dressrosa 11

| | Before | After |
|---|---|---|
| Candidates | 21 | 10 |
| Replacements in the video | 15 | 7, all approved by the read-back |
| Rebecca name card (12:44) | replaced | replaced (both lines) |
| 海 wall (6:01) | replaced | **not replaced**: "The writing moves in a way that could not be followed" |
| `localized_video` | 265.3 s | 265.6 s, 707 MB |

## Open issues

1. **The 海 wall regresses.** The panning shot was eight occurrences (text-001067, -001094,
   -001120, -001144, -001210, -001233 and three Claude boxes, 358.5–362.5 s). Joining them makes
   one occurrence whose frames step between the pieces' boxes and repeat the keyframe box over
   times it did not observe, so following fails where the single piece 001210 was followed and
   replaced before. Joining should keep moving writing apart, or keep each piece's own frames
   and never borrow a box for time it did not see.
2. **2段目 is never detected.** Neither the line nor its だん furigana is in
   `visual/text_detect.json`, and Claude did not add it, so nothing can replace it.
3. **Faint outlines of erased strokes stay.** Colosseum Corps, the だん marks beside Level 4 and
   Level 3, and じょう beside Scrap Yard at 12:55 keep pale outlines of the old writing. The OCR
   read-back does not see them; a local vision model is the next check to try.
4. **Leftover furigana block a replacement.** 幹部塔 at 12:55 (001558) and the bouncing bubbles
   at 2:37.4 (000521, 000524) are refused by the read-back because furigana were not fully
   erased; the check is right, the erase is what falls short.
5. **Huge moving writing can time out.** Following a near full-frame template (the One Piece
   logos while they separated) outran FFmpeg's region read. The logos fall back today, so it is
   latent; a separate task covers it.
6. **A quality-check layout rule fails on Dressrosa 28:** one dialogue cue lasts under 20
   frames. The cue step did not rerun here, so it predates this change and is not from the
   localized video.

The rendered comparison frames (original, before, after) are outside the repository, in the
owner's review folder `tbd-review/m5-polish-2026-09-30/after/sheets/`.

**Status:** live

# Subtitle style rules

The layout, timing and SDH rules every generated subtitle file follows. They are Netflix's English
(USA) timed-text rules, applied to a 24 fps video; where Netflix leaves a choice, the choice made
here is stated. The cue builder in the [pipeline](/documentation/architecture/pipeline.md#9-cue-building)
implements them and the quality check verifies them.

## Text and layout

- At most **42 characters per line** and **two lines** per cue; one line when the text fits.
- Break lines after punctuation, before conjunctions and before prepositions. Never separate an
  article from its noun, an adjective from its noun, a first name from a last name, a subject
  pronoun from its verb, or a phrasal verb from its preposition.
- Prefer a bottom-heavy shape (shorter top line), but never leave one or two words alone on top.
- Reading speed at most **20 characters per second** (adult programmes). Text is never cut to meet
  it; the cue is extended into the gap instead, and the few that still exceed it are reported.

## Timing

- Minimum duration **5/6 s (20 frames)**, maximum **7 s**.
- In-time at speech onset minus 1 frame; out-time at the last word's end plus 12 frames (0.5 s),
  then extended for minimum duration and reading speed. A short cue that can reach the minimum no
  other way may start before its speech, into free time no cue uses, only as far as the minimum
  needs.
- At least **2 frames** between consecutive cues; gaps of 3 to 11 frames are closed to 2 frames
  ("chaining").
- **Shot changes:** a cue whose speech starts within 12 frames after a cut starts on the cut; a
  cue that ends within 12 frames of a cut ends 2 frames before it; no cue straddles a cut by fewer
  than 12 frames. When rules clash: speech covered first, then the 2-frame gap, then minimum
  duration, then shot rules.
- All times sit on frame boundaries of the source video's frame rate.

## Speakers

- Two speakers in one cue: one line each, each line starting with a hyphen and **no space**:

  ```text
  -Are you coming?
  -In a minute.
  ```

- At most one sentence per speaker in a shared cue, but a short interjection too brief for a cue
  of its own may join its speaker's line:

  ```text
  -Oh? Panties?
  -You say you'd like to see them?
  ```
- Speaker labels in brackets (`[Law]`) only when the speaker cannot be identified from the
  picture; the app cannot see the picture, so it adds labels only for voices the language model
  identifies as off-screen from context, sparingly.

## Italics

- Italic for narration, a visible character's unspoken thoughts, voices heard through devices when
  the speaker is not in the scene, and titles of works.
- Never italicise sound cues or speaker labels, even inside italic dialogue.

## SDH sound cues

- In square brackets, lowercase except proper nouns: `[explosion]`, `[crowd cheering]`,
  `[Luffy laughs]`.
- Describe the sound precisely, with adverbs where they help (`[laughs nervously]`), in U.S.
  English.
- Only sounds that matter to the story or mood; never a cue for every grunt in a fight scene.
- Hesitation is written into the dialogue (`I… I said no!`), not labelled `[stutters]`.
- Interjections (`hmm`, `whoa`, `mm`) are kept when they carry meaning; pure filler (`uh`, `um`) is
  dropped.

## Music

- Song lyrics are not transcribed (owner's choice); a song gets one cue such as
  `[upbeat music playing]` or `[theme song playing]`.
- If lyrics are ever enabled: a ♪ at the start and end of each lyric cue, with a space between the
  note and the text; each line capitalised; no comma or period at line ends.

## Output formats

The owner chooses the format in the settings (`output_format`); SRT is the default. One subtitle
file sits beside each video: when the format changes, the file of the old format that the job
wrote moves to the job's `backup/` folder.

- SRT: UTF-8, italics as `<i>…</i>`, no positioning.
- WebVTT: UTF-8, the `WEBVTT` header, italics as `<i>…</i>`, `&`, `<` and `>` as references.
- ASS: UTF-8, one bottom-centred dialogue style (white, black outline), italics as `{\i1}…{\i0}`.
  Sign subtitles (milestone M4) add a sign style positioned near the on-screen text (`\pos`, or
  `{\an8}` for the top).

## On-screen text and the localized video

Netflix moves a subtitle up when it would cover on-screen text. The localized video's subtitle
file, `<video>.localized.ass`, follows that rule; `<video>.ass` and the other formats do not
change.

- `<video>.localized.ass` holds the dialogue and sound cues alone, in the same `Default` style,
  and no on-screen text events: the English is drawn into the localized video, and writing that
  could not be replaced stays Japanese in the picture, never as a second set of subtitles.
- While English lettered into the localized video sits in the bottom band, a cue shown at the
  same time moves to the top (`{\an8}`, 54 pixels below the top edge). The cue's box is
  estimated in the `Default` style: 80 pixels a line on the 1920 × 1080 canvas, about 33 pixels a
  character, centred and at most 1680 pixels wide; the lettered writing counts with 12 pixels
  around it, frame sample by frame sample.
- When writing sits in both bands, the cue takes the band where its box covers less of it; the
  bottom on a tie.

## Sources

- [Netflix English (USA) Timed Text Style Guide](https://partnerhelp.netflixstudios.com/hc/en-us/articles/217350977-English-USA-Timed-Text-Style-Guide)
- [Netflix subtitle timing guidelines](https://partnerhelp.netflixstudios.com/hc/en-us/articles/360051554394-Timed-Text-Style-Guide-Subtitle-Timing-Guidelines)

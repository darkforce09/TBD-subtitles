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
  then extended for minimum duration and reading speed.
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

- At most one sentence per speaker in a shared cue.
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

- SRT: UTF-8, italics as `<i>…</i>`, no positioning.
- ASS: used when positioned sign subtitles exist (milestone M4); dialogue style at the bottom,
  sign style positioned near the on-screen text (`\pos`, or `{\an8}` for the top).

## Sources

- [Netflix English (USA) Timed Text Style Guide](https://partnerhelp.netflixstudios.com/hc/en-us/articles/217350977-English-USA-Timed-Text-Style-Guide)
- [Netflix subtitle timing guidelines](https://partnerhelp.netflixstudios.com/hc/en-us/articles/360051554394-Timed-Text-Style-Guide-Subtitle-Timing-Guidelines)

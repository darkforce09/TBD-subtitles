//! The AudioSet classes the subtitles care about, looked up by name in the rated label set.

use soundevents_dataset::RatedSoundEvent;

use super::ClassRule;

/// Effects, heard on the background stem.
pub const BACKGROUND: &[(&str, f32, f64)] = &[
    ("Explosion", 0.4, 0.5),
    ("Gunshot, gunfire", 0.4, 0.3),
    ("Shatter", 0.4, 0.3),
    ("Thunder", 0.4, 0.5),
    ("Splash, splatter", 0.4, 0.5),
    ("Crowd", 0.5, 1.5),
    ("Cheering", 0.5, 1.0),
    ("Applause", 0.5, 1.0),
    ("Walk, footsteps", 0.5, 1.0),
    ("Door", 0.5, 0.3),
    ("Knock", 0.5, 0.3),
    ("Music", 0.5, 3.0),
];

/// Voices that are not words, heard on the vocal stem.
pub const VOCALS: &[(&str, f32, f64)] = &[
    ("Laughter", 0.4, 0.5),
    ("Screaming", 0.4, 0.5),
    ("Gasp", 0.4, 0.3),
    ("Grunt", 0.4, 0.3),
    ("Groan", 0.4, 0.5),
    ("Sigh", 0.4, 0.5),
    ("Crying, sobbing", 0.4, 1.0),
    ("Whimper", 0.4, 0.5),
    ("Shout", 0.5, 0.5),
    ("Singing", 0.5, 3.0),
];

/// Rules for `table`, each with its index in the 527-class output; `Err` names a class the label
/// set lacks.
pub fn rules(table: &[(&'static str, f32, f64)]) -> Result<Vec<ClassRule>, String> {
    table
        .iter()
        .map(|&(label, threshold, min_s)| {
            let event = RatedSoundEvent::from_key(label)
                .first()
                .ok_or_else(|| format!("AudioSet has no rated class {label:?}"))?;
            Ok(ClassRule {
                label,
                index: event.index(),
                threshold,
                min_s,
            })
        })
        .collect()
}

/// The index of a rated class by name.
pub fn index_of(label: &str) -> Option<usize> {
    RatedSoundEvent::from_key(label).first().map(|e| e.index())
}

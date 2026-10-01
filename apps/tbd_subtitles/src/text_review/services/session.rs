//! Loading visual review state and persisting the owner's corrections.
//!
//! **Role:** join visual results with the job, probe, output, current corrections and, for a job
//! that writes a localized video, its replacements and the selected occurrence's pictures.
//! **Position:** called off the window thread by application actions; depends on plain models.
//! **Signals and state:** reads the job's rows in one snapshot of its database, and its crops;
//! changes the on-screen text corrections in one write transaction of it.
//! **Invariants:** a change rereads the corrections inside its own write transaction, so a change
//! committed meanwhile is kept; thumbnails have bounded total size;
//! absent visual results never masquerade as a completed scan.

use std::path::{Component, Path};

use job_model::StepName;
use job_model::job::JobRecord;
use job_model::onscreen::{TextCorrections, TextDocument, TextEdit, TextOccurrence};
use job_model::outputs::{OutputRecord, ProbeDecoded};
use pipeline::work_dir::{self, JobStore, WorkDir};

use super::{localized, player};
use crate::text_review::models::{Event, LocalizedReview, Picture, ReplacementPictures, Session};

const THUMBNAIL_EDGE: u32 = 128;
const THUMBNAIL_BUDGET: usize = 32 * 1024 * 1024;

/// Read one job's visual results, with bounded representative thumbnails.
pub(crate) fn load(work: &Path) -> Result<Session, String> {
    let Stored {
        document,
        job,
        probe,
        output,
        corrections,
        localized,
    } = stored(work)?;
    if !Path::new(&output.path).is_file() {
        return Err(format!(
            "The exported ASS file is missing: {}. Run this video again to regenerate it.",
            output.path
        ));
    }
    let video = probe
        .probe
        .video
        .as_ref()
        .ok_or("This job has no video picture.")?;
    let fps = video
        .fps()
        .filter(|fps| fps.is_finite() && *fps > 0.0)
        .ok_or("This video's frame rate is unavailable.")?;
    if !probe.probe.duration_s.is_finite() || probe.probe.duration_s <= 0.0 {
        return Err("This video's duration is unavailable.".into());
    }
    let selected = document
        .occurrences
        .iter()
        .position(|text| !text.reviewed && (!text.warnings.is_empty() || text.english.is_none()))
        .unwrap_or(0);
    let draft = document.occurrences.get(selected).map(|text| {
        corrections
            .edits
            .get(&text.id)
            .filter(|edit| edit.matches_source(text))
            .cloned()
            .unwrap_or_else(|| TextEdit::from_occurrence(text))
    });
    let position_s = draft.as_ref().map_or(0.0, |draft| draft.start_s);
    let mut held = 0;
    let thumbnails = document
        .occurrences
        .iter()
        .map(|text| {
            if held + (THUMBNAIL_EDGE * THUMBNAIL_EDGE * 3) as usize > THUMBNAIL_BUDGET {
                return None;
            }
            let thumbnail = text.crops.iter().find_map(|path| thumbnail(work, path));
            if let Some(picture) = &thumbnail {
                held += picture.rgb.len();
            }
            thumbnail
        })
        .collect();
    let mut localized = localized::load(work, localized, &job, &output, &document);
    if let Some(review) = localized.as_mut() {
        review.pictures = selected_pictures(review, &document, selected);
    }
    Ok(Session {
        localized,
        work: work.into(),
        video: job.video.into(),
        ass: output.path.into(),
        flagged_only: document.summary().flagged > 0,
        document,
        corrections,
        duration_s: probe.probe.duration_s,
        fps,
        audio_position: probe.track.audio_position,
        selected,
        draft,
        position_s,
        error: None,
        thumbnails,
    })
}

/// Save, keep, undo or request another reading of the selected occurrence.
pub(crate) fn save(session: &Session, event: &Event) -> Result<(), String> {
    let selected = session.document.occurrences.get(session.selected);
    let edit = match event {
        Event::Save => {
            let selected = selected.ok_or("Select an occurrence before changing it.")?;
            let edit = session
                .draft
                .clone()
                .unwrap_or_else(|| TextEdit::from_occurrence(selected));
            if !edit.matches_source(selected) {
                return Err("This correction does not match the current source text or has no source identity. Reload Check Text and enter the correction again. If the source identity is still unavailable, run the video again before saving.".into());
            }
            edit.validate(session.duration_s)?;
            if edit.presentation.anchor.is_some_and(|anchor| {
                !(0.0..=f64::from(session.document.width)).contains(&anchor.x)
                    || !(0.0..=f64::from(session.document.height)).contains(&anchor.y)
            }) {
                return Err("Text position must be inside the video picture.".into());
            }
            Some(edit)
        }
        Event::Undo | Event::Retry => {
            selected.ok_or("Select an occurrence before changing it.")?;
            None
        }
        Event::DiscardOrphans => None,
        _ => return Err("This action does not save an on-screen text correction.".into()),
    };
    let store = JobStore::open_existing(&WorkDir::new(&session.work))
        .map_err(|error| format!("Cannot save the correction: {error}"))?;
    work_dir::update_text_corrections(&store, |current| {
        change(current, session, selected, edit, event)
    })
    .map_err(|error| format!("Cannot save the correction: {error}"))?;
    Ok(())
}

/// Apply `event` on the selected occurrence `selected` to the stored `current` corrections.
fn change(
    current: &mut TextCorrections,
    session: &Session,
    selected: Option<&TextOccurrence>,
    edit: Option<TextEdit>,
    event: &Event,
) {
    match event {
        Event::Save => {
            current.edits.insert(
                selected.expect("validated selection").id.clone(),
                edit.expect("save has an edit"),
            );
        }
        Event::Undo => {
            current
                .edits
                .remove(&selected.expect("validated selection").id);
        }
        // Each request changes the fingerprint, including another retry of the same occurrence.
        Event::Retry => current
            .retry
            .push(selected.expect("validated selection").id.clone()),
        Event::DiscardOrphans => current.edits.retain(|id, _| {
            session
                .document
                .occurrences
                .iter()
                .any(|text| &text.id == id)
        }),
        _ => unreachable!("validated correction action"),
    }
}

/// The pictures of occurrence `index`'s replacement, when it has one.
pub(crate) fn selected_pictures(
    review: &LocalizedReview,
    document: &TextDocument,
    index: usize,
) -> Option<ReplacementPictures> {
    let id = &document.occurrences.get(index)?.id;
    let replacement = review.replacements.get(id)?;
    Some(localized::pictures(id, replacement))
}

/// What Check Text reads from the job's database.
struct Stored {
    document: TextDocument,
    job: JobRecord,
    probe: ProbeDecoded,
    output: OutputRecord,
    corrections: TextCorrections,
    localized: localized::Rows,
}

/// The rows of the job in `work` Check Text reads, in one snapshot: the typeset text (the reviewed
/// text before typesetting ran), the job record, the probe, the output record, the owner's text
/// corrections and the localized video's rows. An error says in the owner's words what is missing.
fn stored(work: &Path) -> Result<Stored, String> {
    const NO_TEXT: &str = "This job has no on-screen text results. Enable on-screen translation in Settings and run the video again.";
    let rows = work_dir::read_stored(work, |read| {
        let document = match read.output::<TextDocument>(StepName::TextTypeset, None)? {
            Some(document) => Some(document),
            None => read.output(StepName::TextReview, None)?,
        };
        Ok((
            document,
            read.job_record()?,
            read.output::<ProbeDecoded>(StepName::ProbeDecode, None)?,
            read.output::<OutputRecord>(StepName::Output, None),
            read.text_corrections(),
            localized::rows(read),
        ))
    })
    .map_err(|error| error.to_string())?;
    let (document, job, probe, output, corrections, localized) = rows.ok_or(NO_TEXT)?;
    let missing = |what: &str| format!("This job has no {what}. Run this video again.");
    let output = output
        .map_err(|error| error.to_string())
        .and_then(|output| output.ok_or_else(|| "it has not been written".to_string()));
    Ok(Stored {
        document: document.ok_or(NO_TEXT)?,
        job: job.ok_or_else(|| missing("job record"))?,
        probe: probe.ok_or_else(|| missing("probe of its video"))?,
        output: output.map_err(|error| {
            format!("The exported subtitle record is unavailable. Finish or rerun this video to generate its combined ASS file. {error}")
        })?,
        corrections: corrections
            .map_err(|error| format!("Cannot read visual corrections: {error}"))?,
        localized,
    })
}

fn thumbnail(work: &Path, relative: &Path) -> Option<Picture> {
    if relative
        .components()
        .any(|part| !matches!(part, Component::Normal(_) | Component::CurDir))
    {
        return None;
    }
    let mut reader = image::ImageReader::open(work.join(relative)).ok()?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    reader.limits(limits);
    let image = reader
        .decode()
        .ok()?
        .thumbnail(THUMBNAIL_EDGE, THUMBNAIL_EDGE)
        .to_rgb8();
    Some(player::picture(
        image.width(),
        image.height(),
        image.into_raw(),
    ))
}

#[cfg(test)]
#[path = "tests/session.rs"]
mod tests;

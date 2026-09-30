//! Record kinds: which type each row of `job.redb` archives, so a row can be checked before it is
//! stored and printed as JSON.
//!
//! **Role:** name the type behind every table and key a step, the runner or the owner writes, check
//! an archive of it with rkyv's bytecheck, and turn an archive back into JSON.
//!
//! **Position:** used by `workers::channel` to check each output a worker sends before it is kept,
//! and by the app's `dump` command to print rows; reads the `job_model` and `subtitle_formats`
//! types every step writes.
//!
//! **Signals and state:** none; one static kind per type.
//!
//! **Invariants:** a key that names no record type is an error, never a guess; an `outputs` key is
//! a step's name, or `<step>/<part>` for a step's second document; `frames` and `readings` have no
//! record type yet; a kind's check and its JSON read the same type.

use job_model::StepName;
use job_model::job::{JobRecord, StepRecord};
use job_model::onscreen::{
    LocalizedVideoRecord, ReplacementDocument, TextCorrections, TextDocument, VerifiedReplacements,
};
use job_model::outputs::{
    AdjudicationPass, Aligned, Corrections, EngineTranscript, OutputRecord, ProbeDecoded, Redecode,
    ShotChanges, SoundCues, SoundEvent, SpeechPlan, Utterance,
};
use job_model::report::QcReport;
use job_model::store::TableLayouts;
use rkyv::api::high::{HighDeserializer, HighValidator};
use rkyv::bytecheck::CheckBytes;
use rkyv::rancor::Error as ArchiveError;
use rkyv::{Archive, Deserialize};
use subtitle_formats::cue::CueTrack;
use worker_channel::address::{Key, Table};

use crate::error::{PipelineError, Result};

/// The type one row archives: its name, how to check an archive of it, and how to print one.
pub struct RecordKind {
    /// The type's name, such as `ProbeDecoded`.
    pub name: &'static str,
    check: fn(&[u8]) -> std::result::Result<(), String>,
    json: fn(&[u8]) -> std::result::Result<serde_json::Value, String>,
}

impl RecordKind {
    const fn of<T>(name: &'static str) -> RecordKind
    where
        T: Archive + serde::Serialize,
        T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
            + Deserialize<T, HighDeserializer<ArchiveError>>,
    {
        RecordKind {
            name,
            check: check::<T>,
            json: json::<T>,
        }
    }

    /// Whether `bytes` are a valid archive of this kind's type.
    pub fn check(&self, bytes: &[u8]) -> Result<()> {
        (self.check)(bytes).map_err(|error| self.error(error))
    }

    /// The archive in `bytes` as JSON, checked first.
    pub fn json(&self, bytes: &[u8]) -> Result<serde_json::Value> {
        (self.json)(bytes).map_err(|error| self.error(error))
    }

    fn error(&self, error: String) -> PipelineError {
        PipelineError::new(format!("an archive of {}", self.name), error)
    }
}

impl std::fmt::Debug for RecordKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RecordKind")
            .field("name", &self.name)
            .finish()
    }
}

fn check<T>(bytes: &[u8]) -> std::result::Result<(), String>
where
    T: Archive,
    T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>,
{
    rkyv::access::<T::Archived, ArchiveError>(bytes)
        .map(drop)
        .map_err(|error| format!("it does not check: {error}"))
}

fn json<T>(bytes: &[u8]) -> std::result::Result<serde_json::Value, String>
where
    T: Archive + serde::Serialize,
    T::Archived: for<'a> CheckBytes<HighValidator<'a, ArchiveError>>
        + Deserialize<T, HighDeserializer<ArchiveError>>,
{
    let value = rkyv::from_bytes::<T, ArchiveError>(bytes)
        .map_err(|error| format!("it does not check: {error}"))?;
    serde_json::to_value(&value).map_err(|error| format!("it has no JSON form: {error}"))
}

static JOB_RECORD: RecordKind = RecordKind::of::<JobRecord>("JobRecord");
static TABLE_LAYOUTS: RecordKind = RecordKind::of::<TableLayouts>("TableLayouts");
static STEP_RECORD: RecordKind = RecordKind::of::<StepRecord>("StepRecord");
static CORRECTIONS: RecordKind = RecordKind::of::<Corrections>("Corrections");
static TEXT_CORRECTIONS: RecordKind = RecordKind::of::<TextCorrections>("TextCorrections");
static PROBE_DECODED: RecordKind = RecordKind::of::<ProbeDecoded>("ProbeDecoded");
static SHOT_CHANGES: RecordKind = RecordKind::of::<ShotChanges>("ShotChanges");
static SPEECH_PLAN: RecordKind = RecordKind::of::<SpeechPlan>("SpeechPlan");
static ENGINE_TRANSCRIPT: RecordKind = RecordKind::of::<EngineTranscript>("EngineTranscript");
static SHEET: RecordKind = RecordKind::of::<Vec<Utterance>>("Vec<Utterance>");
static SOUND_EVENTS: RecordKind = RecordKind::of::<Vec<SoundEvent>>("Vec<SoundEvent>");
static ADJUDICATION_PASS: RecordKind = RecordKind::of::<AdjudicationPass>("AdjudicationPass");
static REDECODE: RecordKind = RecordKind::of::<Redecode>("Redecode");
static SOUND_CUES: RecordKind = RecordKind::of::<SoundCues>("SoundCues");
static ALIGNED: RecordKind = RecordKind::of::<Aligned>("Aligned");
static CUE_TRACK: RecordKind = RecordKind::of::<CueTrack>("CueTrack");
static DROPPED_SOUNDS: RecordKind = RecordKind::of::<Vec<String>>("Vec<String>");
static TEXT_DOCUMENT: RecordKind = RecordKind::of::<TextDocument>("TextDocument");
static REPLACEMENT_DOCUMENT: RecordKind =
    RecordKind::of::<ReplacementDocument>("ReplacementDocument");
static VERIFIED_REPLACEMENTS: RecordKind =
    RecordKind::of::<VerifiedReplacements>("VerifiedReplacements");
static QC_REPORT: RecordKind = RecordKind::of::<QcReport>("QcReport");
static OUTPUT_RECORD: RecordKind = RecordKind::of::<OutputRecord>("OutputRecord");
static LOCALIZED_VIDEO_RECORD: RecordKind =
    RecordKind::of::<LocalizedVideoRecord>("LocalizedVideoRecord");

/// The kind of the row of `key` in `table`; an error when no record type is defined for it.
pub fn kind(table: Table, key: &Key) -> Result<&'static RecordKind> {
    let at = format!("table {table}, key {}", shown(key));
    if matches!(table, Table::Frames | Table::Readings) {
        return Err(PipelineError::new(
            at,
            format!("no record type is defined for the {table} table yet"),
        ));
    }
    let Key::Name(name) = key else {
        return Err(PipelineError::new(
            at,
            "a per-frame key addresses a table keyed by name",
        ));
    };
    let found = match table {
        Table::Meta => match name.as_str() {
            "job_record" => Some(&JOB_RECORD),
            "layout" => Some(&TABLE_LAYOUTS),
            _ => None,
        },
        Table::StepRecords => name.parse::<StepName>().ok().map(|_| &STEP_RECORD),
        Table::Corrections => match name.as_str() {
            "lines" => Some(&CORRECTIONS),
            "text" => Some(&TEXT_CORRECTIONS),
            _ => None,
        },
        Table::Outputs => output(name),
        Table::Frames | Table::Readings => None,
    };
    found.ok_or_else(|| PipelineError::new(at, "no record type is defined for this key"))
}

/// The kind of an `outputs` key: a step's document, or `<step>/<part>` for its second one.
fn output(name: &str) -> Option<&'static RecordKind> {
    let (step, part) = match name.split_once('/') {
        Some((step, part)) => (step, Some(part)),
        None => (name, None),
    };
    let step = step.parse::<StepName>().ok()?;
    match (step, part) {
        (StepName::Cues, Some("dropped_sounds")) => Some(&DROPPED_SOUNDS),
        (_, Some(_)) => None,
        (step, None) => step_document(step),
    }
}

/// The kind of the document `step` writes today; `None` for a step that writes none.
fn step_document(step: StepName) -> Option<&'static RecordKind> {
    Some(match step {
        StepName::ProbeDecode => &PROBE_DECODED,
        StepName::ShotScan => &SHOT_CHANGES,
        StepName::Separation => return None,
        StepName::Vad => &SPEECH_PLAN,
        StepName::AsrParakeet | StepName::AsrWhisper => &ENGINE_TRANSCRIPT,
        StepName::DiffSheet => &SHEET,
        StepName::SoundEvents => &SOUND_EVENTS,
        StepName::Adjudicate | StepName::Readjudicate => &ADJUDICATION_PASS,
        StepName::RedecodeParakeet | StepName::RedecodeWhisper => &REDECODE,
        StepName::SoundCues => &SOUND_CUES,
        StepName::Alignment | StepName::Review => &ALIGNED,
        StepName::Cues => &CUE_TRACK,
        StepName::TextDetect
        | StepName::TextRead
        | StepName::TextTrack
        | StepName::TextTranslate
        | StepName::TextReview
        | StepName::TextTypeset => &TEXT_DOCUMENT,
        StepName::TextMask | StepName::TextInpaint | StepName::TextCompose => &REPLACEMENT_DOCUMENT,
        StepName::TextVerify => &VERIFIED_REPLACEMENTS,
        StepName::Qc => &QC_REPORT,
        StepName::Output => &OUTPUT_RECORD,
        StepName::LocalizedVideo => &LOCALIZED_VIDEO_RECORD,
    })
}

/// A key as the owner writes it: a name, or `<occurrence>/<frame>`.
pub fn shown(key: &Key) -> String {
    match key {
        Key::Name(name) => name.clone(),
        Key::Frame { occurrence, frame } => format!("{occurrence}/{frame}"),
    }
}

#[cfg(test)]
#[path = "tests/kinds.rs"]
mod tests;

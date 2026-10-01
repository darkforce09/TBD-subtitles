//! A finished job's report read from its database: the job record, the step records, the owner's
//! line corrections, Fix It's record, the quality check, the output record, the typeset on-screen
//! text, and for a localized video its record and the replacements the read-back check approved
//! (composition's while that check has not run); and the summary its sidebar row shows.
//!
//! **Role:** find the video's work directory as the pipeline names it, read its rows in one
//! snapshot, and count its problems and lines worth a listen through `line_counts`, and what Fix
//! It did through `fix_result`.
//!
//! **Position:** called by the application when a finished job is selected or ends, and for the
//! summary of every finished row when the window opens and after each run or correction.
//!
//! **Signals and state:** opens the job's database for one read (sharing this process's handle
//! while a job of it runs here); writes nothing.
//!
//! **Invariants:** a missing or broken row is an error naming it, but for the output record, whose
//! path the settings give, the line corrections, which exist only once a line was corrected, Fix
//! It's record, which exists only once Fix It ran, and the localized video's rows, which exist
//! only once its steps ran and never fail the report; a Fix It record counts only while it
//! belongs to the job's re-adjudication as it stands (`FixRecord::is_current`).

use std::path::{Path, PathBuf};

use job_model::StepName;
use job_model::job::{JobRecord, StepRecords};
use job_model::onscreen::{
    LocalizedVideoRecord, ReplacementDocument, TextDocument, VerifiedReplacements,
};
use job_model::outputs::{Corrections, FixRecord, OutputRecord};
use job_model::report::QcReport;
use pipeline::work_dir::WorkDir;
use pipeline::work_dir::store::StoreRead;
use stages::fix_it::items::Answered;

use crate::job_report::models::report::{JobReport, LocalizedOutput};
use crate::job_report::models::summary::RowSummary;
use crate::job_report::services::{fix_result, line_counts};

/// The report of `video`'s job under `work_root`; an error names the row that is missing or
/// broken.
pub(crate) fn load(video: &Path, work_root: &Path) -> Result<JobReport, String> {
    let (video, work_dir) = work_dir(video, work_root)?;
    let stored = stored(&work_dir, Detail::Report)?;
    let record = stored
        .record
        .ok_or_else(|| missing(&work_dir, "the job record"))?;
    let job_steps = stored.steps;
    let qc = stored.qc.ok_or_else(|| missing(&work_dir, "outputs/qc"))?;
    let corrections = stored.corrections;
    let fix = stored.fix;
    let answered = answered(fix.as_ref());
    let output = stored.output;
    let subtitles = output.as_ref().map_or_else(
        || stages::output::subtitle_path(&video, record.settings.effective_output_format()),
        |output| output.path.clone().into(),
    );
    let localized = localized(stored.localized, &record, &job_steps, output.as_ref());
    let steps = StepName::ALL
        .iter()
        .filter_map(|step| {
            job_steps
                .get(step)
                .map(|done| (*step, done.measure.clone()))
        })
        .collect();
    let problems = line_counts::problems(&qc);
    let fix_result = fix
        .as_ref()
        .filter(|fix| answered_any(fix))
        .map(|fix| fix_result::fix_result(fix, &corrections, &problems));
    let visual = if record.settings.onscreen_text.enabled {
        let typeset = stored
            .typeset
            .ok_or_else(|| missing(&work_dir, "outputs/text_typeset"))?;
        Some(typeset.summary())
    } else {
        None
    };
    Ok(JobReport {
        visual,
        localized,
        report_file: work_dir.join("report.md"),
        lines: line_counts::line_counts(&qc, &corrections, &answered),
        fixable: line_counts::fixable(&qc, &corrections, &answered),
        problems,
        fix_result,
        video,
        work_dir,
        subtitles,
        qc,
        corrections,
        steps,
    })
}

/// What the sidebar row of `video`'s finished job under `work_root` says, from the quality check,
/// the corrections and the Fix It record its database holds; an error names what is missing or
/// broken.
pub(crate) fn summary(video: &Path, work_root: &Path) -> Result<RowSummary, String> {
    let (_, work_dir) = work_dir(video, work_root)?;
    let stored = stored(&work_dir, Detail::Summary)?;
    let qc = stored.qc.ok_or_else(|| missing(&work_dir, "outputs/qc"))?;
    let fix = stored.fix;
    Ok(line_counts::summary(
        &qc,
        &stored.corrections,
        &answered(fix.as_ref()),
        fix.as_ref().is_some_and(answered_any),
    ))
}

/// The rows the localized video's steps left; each is `None` while missing or broken.
#[derive(Default)]
struct LocalizedRows {
    written: Option<LocalizedVideoRecord>,
    verified: Option<VerifiedReplacements>,
    composed: Option<ReplacementDocument>,
}

/// What the localized video left, when the job writes one: the occurrences drawn into it once its
/// steps ran (from its record, else from the replacements the read-back check approved, else from
/// the composed ones), and its video and subtitle file while they are there.
fn localized(
    rows: LocalizedRows,
    record: &JobRecord,
    steps: &StepRecords,
    output: Option<&OutputRecord>,
) -> Option<LocalizedOutput> {
    let text = &record.settings.onscreen_text;
    if !(text.enabled && text.localized_video) {
        return None;
    }
    let written = rows.written.filter(|written| written.path.is_some());
    let ran = |step| steps.contains_key(&step);
    let replaced = match &written {
        Some(written) if ran(StepName::LocalizedVideo) => Some(written.replaced),
        _ => rows
            .verified
            .filter(|_| ran(StepName::TextVerify))
            .map(|verified| verified.document)
            .or(rows.composed.filter(|_| ran(StepName::TextCompose)))
            .map(|document| document.baked().count()),
    };
    let present = |path: Option<String>| path.map(PathBuf::from).filter(|path| path.is_file());
    Some(LocalizedOutput {
        replaced,
        video: present(written.and_then(|written| written.path)),
        subtitles: present(output.and_then(|output| output.localized.clone())),
    })
}

/// The canonical video and its job's work directory, as the pipeline names it.
fn work_dir(video: &Path, work_root: &Path) -> Result<(PathBuf, PathBuf), String> {
    let video = std::fs::canonicalize(video)
        .map_err(|e| format!("cannot find {}: {e}", video.display()))?;
    let work_dir = work_root.join(pipeline::work_dir::job_id(&video));
    Ok((video, work_dir))
}

/// That the database of the job in `work_dir` holds no `row`.
fn missing(work_dir: &Path, row: &str) -> String {
    format!(
        "{} has no {row}",
        WorkDir::new(work_dir).database().display()
    )
}

/// How much of the job a read needs: the sidebar row's counts, or the whole report.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Detail {
    Summary,
    Report,
}

/// What the job's database holds for its report: the job record, the step records, the owner's
/// corrections, Fix It's record while it belongs to the job as it stands, the quality check, and
/// for the whole report the output record, the typeset text and the localized video's rows.
#[derive(Default)]
struct Stored {
    record: Option<JobRecord>,
    steps: StepRecords,
    corrections: Corrections,
    fix: Option<FixRecord>,
    qc: Option<QcReport>,
    output: Option<OutputRecord>,
    typeset: Option<TextDocument>,
    localized: LocalizedRows,
}

/// The rows of the job in `work_dir` its report reads, none of them for a job with no database;
/// an error when a row that must read does not, or another process runs the job.
fn stored(work_dir: &Path, detail: Detail) -> Result<Stored, String> {
    let stored = pipeline::work_dir::read_stored(work_dir, |read| {
        let mut stored = Stored {
            record: read.job_record()?,
            steps: read.step_records()?,
            corrections: read.line_corrections()?,
            fix: read.fix_record()?,
            qc: read.output(StepName::Qc, None)?,
            ..Stored::default()
        };
        if detail == Detail::Report {
            read_report_rows(read, &mut stored)?;
        }
        Ok(stored)
    })
    .map_err(|e| e.to_string())?;
    let mut stored = stored.unwrap_or_default();
    let steps = &stored.steps;
    stored.fix = stored.fix.take().filter(|fix| fix.is_current(steps));
    Ok(stored)
}

/// Add the rows only the whole report reads: the output record and the localized video's rows,
/// which never fail it, and the typeset text of a job with on-screen text on.
fn read_report_rows(read: &StoreRead, stored: &mut Stored) -> pipeline::Result<()> {
    let text = stored
        .record
        .as_ref()
        .map(|record| record.settings.onscreen_text.clone());
    stored.output = read.output(StepName::Output, None).ok().flatten();
    if text.as_ref().is_some_and(|text| text.enabled) {
        stored.typeset = read.output(StepName::TextTypeset, None)?;
    }
    if text.is_some_and(|text| text.enabled && text.localized_video) {
        stored.localized = LocalizedRows {
            written: read.output(StepName::LocalizedVideo, None).ok().flatten(),
            verified: read.output(StepName::TextVerify, None).ok().flatten(),
            composed: read.output(StepName::TextCompose, None).ok().flatten(),
        };
    }
    Ok(())
}

/// The lines `fix` answered; none without a record.
fn answered(fix: Option<&FixRecord>) -> Answered {
    fix.map_or_else(Answered::none, Answered::from_record)
}

/// Whether `fix`'s runs answered at least one line.
fn answered_any(fix: &FixRecord) -> bool {
    fix.lines.iter().any(|line| line.verdict.answered())
}

#[cfg(test)]
#[path = "tests/report_loading.rs"]
mod tests;

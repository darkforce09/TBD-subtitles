//! Rows of a finished job's database as the window's tests put them: the lines Check Lines reads
//! and the quality check.

use std::path::Path;
use std::sync::Arc;

use job_model::StepName;
use job_model::outputs::{AdjudicationPass, Findings, Line, Utterance};
use job_model::report::QcReport;
use pipeline::work_dir::{JobStore, WorkDir};

/// One line of a fixture job: its id, where it starts and how long it lasts, what Parakeet and
/// Whisper heard, and the text and flags the language model settled on.
pub(super) struct Heard<'a> {
    pub(super) id: &'a str,
    pub(super) start_s: f64,
    pub(super) length_s: f64,
    pub(super) parakeet: &'a str,
    pub(super) whisper: &'a str,
    pub(super) settled: &'a str,
    pub(super) flags: &'a [&'a str],
}

/// This process's handle of the database of the job in `job`, created when missing.
pub(super) fn job_store(job: &Path) -> Arc<JobStore> {
    JobStore::open(&WorkDir::new(job)).expect("the store")
}

/// Store `lines` as the sheet and the settled re-adjudication of the job in `job`, as Check Lines
/// reads them.
pub(super) fn put_lines(job: &Path, lines: &[Heard<'_>]) {
    let sheet: Vec<Utterance> = lines
        .iter()
        .map(|line| Utterance {
            id: line.id.into(),
            start_s: line.start_s,
            end_s: line.start_s + line.length_s,
            words: Vec::new(),
            locked: Vec::new(),
            line: line.id.into(),
            hypotheses: vec![
                ("P".into(), vec![line.parakeet.into()]),
                ("W".into(), vec![line.whisper.into()]),
            ],
        })
        .collect();
    let settled = AdjudicationPass {
        lines: lines
            .iter()
            .map(|line| Line {
                id: line.id.into(),
                t: line.settled.into(),
                f: line.flags.iter().map(|flag| flag.to_string()).collect(),
            })
            .collect(),
        findings: Findings::default(),
        redecoded: Vec::new(),
        calls: 1,
        input_tokens: 0,
        output_tokens: 0,
        cost_usd: 0.0,
        failed_calls: Vec::new(),
    };
    let store = job_store(job);
    store
        .put_output(StepName::DiffSheet, None, &sheet)
        .expect("the sheet");
    store
        .put_output(StepName::Readjudicate, None, &settled)
        .expect("the adjudication");
}

/// Store `qc` as the quality check of the job in `job`.
pub(super) fn put_qc(job: &Path, qc: &QcReport) {
    job_store(job)
        .put_output(StepName::Qc, None, qc)
        .expect("the quality check");
}

/// The quality check the job in `job` stores.
pub(super) fn stored_qc(job: &Path) -> QcReport {
    job_store(job)
        .read()
        .expect("read")
        .output(StepName::Qc, None)
        .expect("the quality check reads")
        .expect("the quality check")
}

/// The settled re-adjudication the job in `job` stores.
pub(super) fn stored_settled(job: &Path) -> AdjudicationPass {
    job_store(job)
        .read()
        .expect("read")
        .output(StepName::Readjudicate, None)
        .expect("the adjudication reads")
        .expect("the adjudication")
}

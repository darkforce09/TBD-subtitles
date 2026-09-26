//! The language-model tasks: the first pass over the sheet, and the second pass over the unsure
//! utterances with what the engines heard again.
//!
//! **Role:** run `claude -p` over the sheet (several processes at once), check the answer, and
//! merge the second pass into the first.
//!
//! **Position:** called by `tasks::run` inside a worker of the main binary; uses
//! `stages::adjudication` and `inference::llm::claude_cli`.
//!
//! **Signals and state:** reads `sheet.json`, `adjudication/first.json` and the re-decodes; writes
//! `adjudication/first.json` and `adjudicated.json`; `claude` runs in the job's empty
//! `claude-cwd/`.
//!
//! **Invariants:** a first pass with no answer at all is an error; the second pass replaces only
//! the unsure lines, and the checks run on the merged answer.

use std::time::Instant;

use inference::llm::LanguageModel;
use inference::llm::claude_cli::ClaudeCli;
use job_model::outputs::{AdjudicationPass, Redecode, Utterance};
use stages::adjudication::{self, Adjudication, checks, redecode};

use super::{Job, StepProgress, TaskReport, since};
use crate::error::Result;
use crate::work_dir;

/// A factory of `claude -p` backends for this job, one per concurrent process.
pub(super) fn claude(job: &Job) -> impl Fn() -> Box<dyn LanguageModel + Send> + Sync + use<> {
    let model = job.settings().llm_model.clone();
    let cwd = job.work.claude_cwd();
    move || Box::new(ClaudeCli::new(&model, cwd.clone())) as Box<dyn LanguageModel + Send>
}

pub(super) fn adjudicate(job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let sheet: Vec<Utterance> = work_dir::read_json(&job.work.sheet())?;
    let glossary = job.glossary();
    let started = Instant::now();
    let make = claude(job);
    let result = adjudication::adjudicate_concurrently(
        &make,
        job.settings().llm_processes,
        &sheet,
        &glossary,
        progress,
    );
    let findings = checks::check(&sheet, &result.lines, &glossary);
    let pass = to_pass(result, findings, Vec::new());
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    notes(&mut report, &pass);
    if pass.lines.is_empty() && !sheet.is_empty() {
        return Err(crate::error::PipelineError::new(
            "adjudicate",
            format!("no answer: {:?}", pass.failed_calls),
        ));
    }
    work_dir::write_json(&job.work.first_pass(), &pass)?;
    Ok(report)
}

pub(super) fn readjudicate(job: &Job, progress: StepProgress) -> Result<TaskReport> {
    let sheet: Vec<Utterance> = work_dir::read_json(&job.work.sheet())?;
    let first: AdjudicationPass = work_dir::read_json(&job.work.first_pass())?;
    let parakeet: Redecode = work_dir::read_json(&job.work.redecode("parakeet"))?;
    let whisper: Redecode = work_dir::read_json(&job.work.redecode("whisper"))?;
    let glossary = job.glossary();
    let started = Instant::now();
    let ids = redecode::unsure_ids(&first.lines);
    let with_alternatives =
        redecode::with_alternatives(&sheet, &[("p", &parakeet), ("w", &whisper)]);
    let mut pass = first.clone();
    if !ids.is_empty() {
        let mut model = claude(job)();
        let second = redecode::readjudicate(
            model.as_mut(),
            &with_alternatives,
            &first.lines,
            &ids,
            &glossary,
            progress,
        );
        pass.lines = redecode::merge(&first.lines, &second.lines);
        pass.findings = checks::check(&with_alternatives, &pass.lines, &glossary);
        pass.redecoded = ids;
        pass.calls += second.calls;
        pass.input_tokens += second.input_tokens;
        pass.output_tokens += second.output_tokens;
        pass.cost_usd += second.cost_usd;
        pass.failed_calls.extend(second.failed_calls);
    }
    let mut report = TaskReport {
        process_s: since(started),
        ..TaskReport::default()
    };
    notes(&mut report, &pass);
    report.note("redecoded", pass.redecoded.len());
    work_dir::write_json(&job.work.adjudicated(), &pass)?;
    Ok(report)
}

fn to_pass(
    result: Adjudication,
    findings: job_model::outputs::Findings,
    redecoded: Vec<String>,
) -> AdjudicationPass {
    AdjudicationPass {
        lines: result.lines,
        findings,
        redecoded,
        calls: result.calls,
        input_tokens: result.input_tokens,
        output_tokens: result.output_tokens,
        cost_usd: result.cost_usd,
        failed_calls: result.failed_calls,
    }
}

fn notes(report: &mut TaskReport, pass: &AdjudicationPass) {
    let flagged = |flag: &str| pass.lines.iter().filter(|l| l.has_flag(flag)).count();
    report.note("calls", pass.calls);
    report.note("failed_calls", pass.failed_calls.len());
    report.note("cost_usd", format!("{:.2}", pass.cost_usd));
    report.note("unsure", flagged("UNSURE"));
    report.note("lyric", flagged("LYRIC"));
    report.note("drop", flagged("DROP"));
    report.note("novel", pass.findings.novel.len());
    report.note("removed_locked", pass.findings.removed_locked.len());
    report.note("missing_ids", pass.findings.missing_ids.len());
}

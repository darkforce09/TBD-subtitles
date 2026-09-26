//! The quality check: the finished cues held against the layout, timing and coverage rules, and
//! the lines the earlier steps flagged, each with a timestamp to look at.
//!
//! **Role:** list every cue that breaks a rule (overlap, gap, duration, reading speed, line
//! length, line count, empty, past the end); every stretch of detected speech over 1 s outside a
//! song with no cue; every unsure, novel or dropped-agreed-word line; every utterance timed
//! without the aligner; the aligner's offset; failed model calls; and the counts that sum it up.
//!
//! **Position:** called by the QC step with the job's outputs; `markdown.rs` renders the result
//! with the step timings as `report.md`.
//!
//! **Signals and state:** none; pure.
//!
//! **Invariants:** findings come out in time order; a check that has nothing to look at reports
//! nothing rather than success in its place (the summary shows what was counted).

pub mod coverage;
pub mod markdown;

use std::collections::{BTreeMap, HashMap};

use job_model::outputs::{
    AdjudicationPass, Aligned, SoundCues, SpeechPlan, TimeSpan, TimingSource,
};
use job_model::report::{QcCheck, QcFinding, QcReport, QcSummary};
use subtitle_formats::cue::{CueKind, CueTrack};

use crate::cues::FrameRules;
use crate::cues::line_break::MAX_LINE;
use crate::cues::segment::MAX_CPS;

/// Target share of cues at or under 20 characters per second.
pub const CPS_TARGET: f64 = job_model::report::CPS_TARGET;
/// An aligner offset at or over this is reported.
pub const MAX_OFFSET_S: f64 = 0.030;
/// An utterance with less than this share of aligner-timed words is reported.
pub const MIN_ALIGNED_SHARE: f64 = 0.5;

/// Everything the check reads.
pub struct QcInput<'a> {
    pub track: &'a CueTrack,
    pub aligned: &'a Aligned,
    pub adjudicated: &'a AdjudicationPass,
    pub sound_cues: &'a SoundCues,
    pub speech: &'a SpeechPlan,
    /// Stretches where an engine heard words (`coverage::heard_spans`).
    pub heard: &'a [TimeSpan],
    /// Utterances dropped as noise: detected speech there needs no cue.
    pub dropped: &'a [TimeSpan],
    /// Utterance id to its start, for findings about utterances.
    pub utterance_starts: &'a HashMap<String, f64>,
    pub duration_s: f64,
}

/// Run every check.
pub fn check(input: &QcInput) -> QcReport {
    let rules = FrameRules::new(input.track.frame_rate, input.duration_s);
    let mut findings = cue_findings(input.track, &rules);
    attach_utterances(&mut findings, input.track, input.aligned);
    let excused: Vec<TimeSpan> = input
        .sound_cues
        .candidates
        .iter()
        .filter(|c| c.kind == job_model::outputs::CandidateKind::Song)
        .map(|c| TimeSpan::new(c.start_s, c.end_s))
        .chain(input.dropped.iter().copied())
        .collect();
    let voice_without_cue_s = coverage::uncovered(&input.speech.regions, &excused, input.track)
        .iter()
        .map(TimeSpan::duration_s)
        .sum::<f64>()
        .max(0.0);
    let uncovered = coverage::uncovered(input.heard, &excused, input.track);
    // `max` turns an empty sum, -0.0, into 0.0 for the report.
    let uncovered_speech_s = uncovered
        .iter()
        .map(TimeSpan::duration_s)
        .sum::<f64>()
        .max(0.0);
    for span in uncovered {
        findings.push(QcFinding {
            check: QcCheck::UncoveredSpeech,
            time_s: span.start_s,
            text: String::new(),
            detail: format!("{:.1} s", span.duration_s()),
            utterance: None,
        });
    }
    findings.extend(line_findings(input));
    findings.sort_by(|a, b| a.time_s.total_cmp(&b.time_s).then(a.check.cmp(&b.check)));
    let mut summary = summary(input, &rules, uncovered_speech_s);
    summary.voice_without_cue_s = voice_without_cue_s;
    QcReport { summary, findings }
}

/// The rules each cue and each pair of neighbouring cues must keep.
pub fn cue_findings(track: &CueTrack, rules: &FrameRules) -> Vec<QcFinding> {
    let rate = track.frame_rate;
    let mut out = Vec::new();
    let mut flag = |check, cue: &subtitle_formats::cue::Cue, detail: String| {
        out.push(QcFinding {
            check,
            time_s: rate.seconds(cue.start),
            text: cue.text(),
            detail,
            utterance: None,
        });
    };
    for (i, cue) in track.cues.iter().enumerate() {
        if cue.lines.iter().all(|l| l.text.trim().is_empty()) {
            flag(QcCheck::Empty, cue, String::new());
        }
        if cue.lines.len() > 2 {
            flag(
                QcCheck::TooManyLines,
                cue,
                format!("{} lines", cue.lines.len()),
            );
        }
        if let Some(longest) = cue
            .lines
            .iter()
            .map(|l| l.chars())
            .max()
            .filter(|&n| n > MAX_LINE)
        {
            flag(QcCheck::LineTooLong, cue, format!("{longest} characters"));
        }
        if cue.frames() < rules.min {
            flag(QcCheck::TooShort, cue, format!("{} frames", cue.frames()));
        }
        if cue.frames() > rules.max {
            flag(
                QcCheck::TooLong,
                cue,
                format!("{:.2} s", cue.frames() as f64 * rate.frame_s()),
            );
        }
        if cue.cps(rate) > MAX_CPS {
            flag(QcCheck::TooFast, cue, format!("{:.1} cps", cue.cps(rate)));
        }
        if cue.end > rules.total {
            flag(
                QcCheck::PastEnd,
                cue,
                format!("ends at frame {} of {}", cue.end, rules.total),
            );
        }
        if let Some(next) = track.cues.get(i + 1) {
            if next.start < cue.end {
                flag(
                    QcCheck::Overlap,
                    cue,
                    format!("{} frames into the next cue", cue.end - next.start),
                );
            } else if next.start - cue.end < rules.gap {
                flag(
                    QcCheck::GapTooSmall,
                    cue,
                    format!("{} frame gap", next.start - cue.end),
                );
            }
        }
    }
    out
}

/// Name, on each finding about a cue, the utterance whose words overlap the cue the longest, so
/// the owner can open that line for review.
fn attach_utterances(findings: &mut [QcFinding], track: &CueTrack, aligned: &Aligned) {
    let rate = track.frame_rate;
    for finding in findings.iter_mut() {
        let Some(cue) = track
            .cues
            .iter()
            .find(|c| rate.seconds(c.start) == finding.time_s)
        else {
            continue;
        };
        let (from, to) = (rate.seconds(cue.start), rate.seconds(cue.end));
        finding.utterance = aligned
            .utterances
            .iter()
            .filter_map(|u| {
                let start = u.words.first()?.start_s;
                let end = u.words.last()?.end_s;
                let overlap = end.min(to) - start.max(from);
                (overlap > 0.0).then_some((overlap, &u.id))
            })
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, id)| id.clone());
    }
}

/// Findings about utterances and model calls.
fn line_findings(input: &QcInput) -> Vec<QcFinding> {
    let at = |id: &str| input.utterance_starts.get(id).copied().unwrap_or(0.0);
    let text_of: HashMap<&str, &str> = input
        .adjudicated
        .lines
        .iter()
        .map(|l| (l.id.as_str(), l.t.as_str()))
        .collect();
    let text = |id: &str| text_of.get(id).map_or_else(String::new, |t| t.to_string());
    let mut out = Vec::new();
    for u in &input.aligned.utterances {
        let words = u
            .words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if u.unsure {
            out.push(QcFinding {
                check: QcCheck::Unsure,
                time_s: at(&u.id),
                text: words.clone(),
                detail: u.id.clone(),
                utterance: Some(u.id.clone()),
            });
        }
        let aligned = u
            .words
            .iter()
            .filter(|w| matches!(w.source, TimingSource::Ctc | TimingSource::CtcUtterance))
            .count();
        if !u.words.is_empty() && (aligned as f64 / u.words.len() as f64) < MIN_ALIGNED_SHARE {
            out.push(QcFinding {
                check: QcCheck::WeakTiming,
                time_s: u.words[0].start_s,
                text: words,
                detail: format!("{} of {} words aligned", aligned, u.words.len()),
                utterance: Some(u.id.clone()),
            });
        }
    }
    let findings = &input.adjudicated.findings;
    for (id, word) in &findings.novel {
        out.push(QcFinding {
            check: QcCheck::Novel,
            time_s: at(id),
            text: text(id),
            detail: format!("{id}: {word}"),
            utterance: Some(id.clone()),
        });
    }
    for (id, word) in &findings.removed_locked {
        out.push(QcFinding {
            check: QcCheck::RemovedLocked,
            time_s: at(id),
            text: text(id),
            detail: format!("{id}: {word}"),
            utterance: Some(id.clone()),
        });
    }
    if let Some(offset) = input.aligned.offset_s.filter(|o| o.abs() >= MAX_OFFSET_S) {
        out.push(QcFinding {
            check: QcCheck::Offset,
            time_s: 0.0,
            text: String::new(),
            detail: format!("{:+.0} ms", offset * 1000.0),
            utterance: None,
        });
    }
    for failure in input
        .adjudicated
        .failed_calls
        .iter()
        .chain(&input.sound_cues.failed_calls)
    {
        out.push(QcFinding {
            check: QcCheck::FailedCall,
            time_s: 0.0,
            text: String::new(),
            detail: failure.clone(),
            utterance: None,
        });
    }
    out
}

fn summary(input: &QcInput, rules: &FrameRules, uncovered_speech_s: f64) -> QcSummary {
    let cues = &input.track.cues;
    let count = |kind| cues.iter().filter(|c| c.kind == kind).count();
    let ok = cues.iter().filter(|c| c.cps(rules.rate) <= MAX_CPS).count();
    let mut words_by_source = BTreeMap::new();
    for source in TimingSource::ALL {
        words_by_source.insert(source.as_str().to_string(), 0);
    }
    for w in input.aligned.utterances.iter().flat_map(|u| &u.words) {
        *words_by_source
            .entry(w.source.as_str().to_string())
            .or_insert(0) += 1;
    }
    QcSummary {
        cues: cues.len(),
        dialogue_cues: count(CueKind::Dialogue),
        sound_cues: count(CueKind::Sound),
        music_cues: count(CueKind::Music),
        cps_ok_share: if cues.is_empty() {
            1.0
        } else {
            ok as f64 / cues.len() as f64
        },
        words_by_source,
        unsure: input.aligned.utterances.iter().filter(|u| u.unsure).count(),
        novel: input.adjudicated.findings.novel.len(),
        offset_ms: input.aligned.offset_s.map(|o| o * 1000.0),
        uncovered_speech_s,
        voice_without_cue_s: 0.0,
        reviewed: 0,
        video_s: input.duration_s,
    }
}

#[cfg(test)]
#[path = "tests/qc.rs"]
mod tests;

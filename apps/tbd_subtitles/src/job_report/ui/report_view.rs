//! A finished job's report: whether it passes the quality check and why not, the counts, the
//! files with buttons to open them, every finding with its time, and each step's time and memory.
//!
//! **Role:** draw the borrowed `JobReport` and turn the buttons into `ReportEvent`s.
//!
//! **Position:** called by the application's Jobs page for a finished job.
//!
//! **Signals and state:** none, but for copying the subtitle path to the clipboard.
//!
//! **Invariants:** a job that fails the quality check says so first, with each failed rule.

use eframe::egui::{RichText, Ui};
use egui_extras::{Column, TableBuilder};

use crate::core::format;
use crate::core::ui::{BAD, GOOD, MUTED_TEXT};
use crate::job_report::events::ReportEvent;
use crate::job_report::models::report::JobReport;

/// Draw `report` and push what the owner asked for onto `events`.
pub(crate) fn report_view_ui(ui: &mut Ui, report: &JobReport, events: &mut Vec<ReportEvent>) {
    let failures = report.qc.failures();
    if failures.is_empty() {
        ui.label(
            RichText::new("✓ Passes the quality check")
                .color(GOOD)
                .strong(),
        );
    } else {
        ui.label(
            RichText::new("✗ Does not pass the quality check")
                .color(BAD)
                .strong(),
        );
        for reason in &failures {
            ui.label(RichText::new(format!("  • {reason}")).color(BAD));
        }
    }
    let s = &report.qc.summary;
    ui.label(format!(
        "{} cues ({} dialogue, {} sound, {} music) · {:.1} % within 20 characters per second",
        s.cues,
        s.dialogue_cues,
        s.sound_cues,
        s.music_cues,
        s.cps_ok_share * 100.0
    ));
    ui.label(
        RichText::new(format!(
            "unsure lines {} · novel words {} · aligner offset {} · heard speech with no cue \
             {:.1} s · voice with no cue {:.1} s",
            s.unsure,
            s.novel,
            s.offset_ms
                .map_or_else(|| "—".to_string(), |ms| format!("{ms:+.0} ms")),
            s.uncovered_speech_s,
            s.voice_without_cue_s
        ))
        .color(MUTED_TEXT),
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label("Subtitles:");
        ui.label(RichText::new(report.subtitles.display().to_string()).monospace());
        if ui.small_button("Copy path").clicked() {
            ui.ctx().copy_text(report.subtitles.display().to_string());
        }
    });
    ui.horizontal(|ui| {
        if ui.button("Open in the video player").clicked() {
            events.push(ReportEvent::Open(report.video.clone()));
        }
        if let Some(folder) = report.video.parent()
            && ui.button("Open the video's folder").clicked()
        {
            events.push(ReportEvent::Open(folder.to_path_buf()));
        }
        if ui.button("Open report.md").clicked() {
            events.push(ReportEvent::Open(report.report_file.clone()));
        }
    });
    ui.add_space(8.0);
    ui.heading(format!("Findings ({})", report.qc.findings.len()));
    findings_ui(ui, report);
    ui.add_space(8.0);
    ui.heading(format!(
        "Steps ({} of step time)",
        format::duration(report.total_s())
    ));
    steps_ui(ui, report);
}

fn findings_ui(ui: &mut Ui, report: &JobReport) {
    if report.qc.findings.is_empty() {
        ui.label(RichText::new("No findings.").color(MUTED_TEXT));
        return;
    }
    ui.push_id("findings", |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .vscroll(false)
            .column(Column::auto().at_least(70.0))
            .column(Column::auto().at_least(160.0))
            .column(Column::initial(380.0).clip(true))
            .column(Column::remainder())
            .header(20.0, |mut header| {
                for title in ["Time", "Check", "Text", "Detail"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|mut body| {
                for finding in &report.qc.findings {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            ui.monospace(clock(finding.time_s));
                        });
                        row.col(|ui| {
                            ui.label(finding.check.describe());
                        });
                        row.col(|ui| {
                            ui.label(&finding.text).on_hover_text(&finding.text);
                        });
                        row.col(|ui| {
                            ui.label(RichText::new(&finding.detail).color(MUTED_TEXT));
                        });
                    });
                }
            });
    });
}

fn steps_ui(ui: &mut Ui, report: &JobReport) {
    let mib = |v: Option<f64>| v.map_or_else(|| "—".to_string(), |v| format!("{v:.0} MiB"));
    ui.push_id("steps", |ui| {
        TableBuilder::new(ui)
            .striped(true)
            .vscroll(false)
            .columns(Column::auto().at_least(120.0), 4)
            .header(20.0, |mut header| {
                for title in ["Step", "Time", "Peak RAM", "Peak VRAM"] {
                    header.col(|ui| {
                        ui.strong(title);
                    });
                }
            })
            .body(|mut body| {
                for (step, measure) in &report.steps {
                    body.row(20.0, |mut row| {
                        row.col(|ui| {
                            ui.label(step.as_str());
                        });
                        row.col(|ui| {
                            ui.label(format::duration(measure.wall_s));
                        });
                        row.col(|ui| {
                            ui.label(mib(measure
                                .peak_ram_mib
                                .into_iter()
                                .chain(measure.peak_child_ram_mib)
                                .reduce(f64::max)));
                        });
                        row.col(|ui| {
                            ui.label(mib(measure.peak_vram_mib));
                        });
                    });
                }
            });
    });
}

/// A video time as `h:mm:ss.d`, as `report.md` writes it.
fn clock(seconds: f64) -> String {
    let tenths = (seconds.max(0.0) * 10.0).round() as u64;
    format!(
        "{}:{:02}:{:02}.{}",
        tenths / 36_000,
        tenths / 600 % 60,
        tenths / 10 % 60,
        tenths % 10
    )
}

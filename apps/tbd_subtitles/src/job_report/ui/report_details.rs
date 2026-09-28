//! The Overview's two disclosures: Details (the quality check's numbers in plain words) and Step
//! times (each stage and step with its time, peak RAM and peak VRAM, and Open Full Report).
//!
//! **Role:** draw the borrowed report's numbers, folded away until the owner opens them, and turn
//! Open Full Report into a `ReportEvent`.
//!
//! **Position:** the last two cards of `overview`; the rows open through `core::ui::disclosure`.
//!
//! **Signals and state:** whether each disclosure is open lives in egui's memory, per job.
//!
//! **Invariants:** both start closed; a step with no measure shows "—"; the total leaves out the
//! shot scan, which runs alongside the other steps.

use std::sync::Arc;

use eframe::egui::{
    Align2, Color32, FontFamily, FontId, Frame, Galley, Id, Margin, Rect, Sense, Stroke, Ui, pos2,
    vec2,
};
use job_model::StepName;
use job_model::job::StepMeasure;
use job_model::outputs::TimingSource;

use crate::core::format;
use crate::core::steps::{STAGES, step_title};
use crate::core::ui::button::{Button, ButtonSize};
use crate::core::ui::card::card;
use crate::core::ui::disclosure::disclosure;
use crate::core::ui::fonts;
use crate::core::ui::icons;
use crate::core::ui::palette::palette;
use crate::job_report::events::ReportEvent;
use crate::job_report::models::report::JobReport;

/// The space beside a row's content and above and below it, in Details and in Step times.
const SIDE: f32 = 18.0;
const KV_PAD: f32 = 8.0;
const TABLE_PAD: f32 = 6.0;
/// The width of Details' key column, of each number column, and a step's indent.
const KEY_WIDTH: f32 = 220.0;
const NUMBER_WIDTH: f32 = 110.0;
const STEP_INDENT: f32 = 34.0;

/// The Details disclosure: the check's counts, easy reading, unsure lines, new words, timing,
/// speech with no subtitle, the aligner's share and the corrections made.
pub(super) fn details_ui(ui: &mut Ui, report: &JobReport) {
    let s = &report.qc.summary;
    let easy = format!("{:.1} %", s.cps_ok_share * 100.0);
    card(ui, false, |ui| {
        let aside = format!("{} subtitles · {easy} easy to read", s.cues);
        let id = Id::new(("report-details", &report.work_dir));
        if !disclosure(ui, id, ("Details", "Details"), &aside) {
            return;
        }
        let words: usize = s.words_by_source.values().sum();
        let aligned: usize = [TimingSource::Ctc, TimingSource::CtcUtterance]
            .iter()
            .filter_map(|source| s.words_by_source.get(source.as_str()))
            .sum();
        let offset = s.offset_ms.map_or_else(
            || "not measured".to_string(),
            |ms| format!("{ms:+.0} ms (limit 30 ms)"),
        );
        let rows = [
            (
                "Subtitles",
                format!(
                    "{} ({} dialogue, {} sound, {} music)",
                    s.cues, s.dialogue_cues, s.sound_cues, s.music_cues
                ),
            ),
            (
                "Easy to read",
                format!("{easy} within 20 characters per second (target 95 %)"),
            ),
            ("Unsure lines", s.unsure.to_string()),
            ("Words no engine heard", s.novel.to_string()),
            ("Timing offset", offset),
            (
                "Speech with no subtitle",
                format!("{:.1} s", s.uncovered_speech_s),
            ),
            (
                "Voice with no subtitle",
                format!(
                    "{:.1} s, mostly songs and background voices",
                    s.voice_without_cue_s
                ),
            ),
            (
                "Words timed by the aligner",
                format!("{aligned} of {words}"),
            ),
            (
                "Corrections you made",
                report.corrections.lines.len().to_string(),
            ),
        ];
        for (key, value) in rows {
            key_value_ui(ui, key, &value);
        }
    });
}

/// One row of Details: the key in grey, its value beside it, a line above.
fn key_value_ui(ui: &mut Ui, key: &str, value: &str) {
    let p = palette(ui);
    let width = ui.available_width();
    let value_width = (width - KEY_WIDTH - 2.0 * SIDE).max(40.0);
    let key = text(ui, key, p.text2, KEY_WIDTH - SIDE);
    let value = text(ui, value, p.text, value_width);
    let height = 2.0 * KV_PAD + key.size().y.max(value.size().y);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, p.line));
    let top = rect.top() + KV_PAD;
    ui.painter()
        .galley(pos2(rect.left() + SIDE, top), key, p.text2);
    ui.painter()
        .galley(pos2(rect.left() + KEY_WIDTH + SIDE, top), value, p.text);
}

/// The Step times disclosure: a table of the six stages, each with its steps, then Open Full
/// Report.
pub(super) fn step_times_ui(ui: &mut Ui, report: &JobReport, events: &mut Vec<ReportEvent>) {
    card(ui, false, |ui| {
        let aside = format!(
            "{} in total, shot scan aside",
            format::duration(report.total_s())
        );
        let id = Id::new(("report-steps", &report.work_dir));
        if !disclosure(ui, id, ("Step times", "Step times"), &aside) {
            return;
        }
        let p = palette(ui);
        table_row_ui(
            ui,
            ["Step", "Time", "Peak RAM", "Peak VRAM"].map(str::to_string),
            Row::Header,
        );
        let measure = |step: StepName| {
            report
                .steps
                .iter()
                .find(|(done, _)| *done == step)
                .map(|(_, measure)| measure)
        };
        for stage in &STAGES {
            let measures: Vec<&StepMeasure> = stage
                .steps
                .iter()
                .filter_map(|step| measure(*step))
                .collect();
            let total = (!measures.is_empty())
                .then(|| format::duration(measures.iter().map(|m| m.wall_s).sum()));
            table_row_ui(
                ui,
                [
                    stage.title.to_string(),
                    total.unwrap_or_else(|| "—".to_string()),
                    String::new(),
                    String::new(),
                ],
                Row::Stage,
            );
            for step in stage.steps {
                table_row_ui(ui, step_cells(*step, measure(*step)), Row::Step);
            }
        }
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter()
            .hline(rect.x_range(), rect.top(), Stroke::new(1.0, p.line));
        Frame::new()
            .inner_margin(Margin::symmetric(18, 10))
            .show(ui, |ui| {
                let open = Button::new("Open Full Report")
                    .icon(icons::FILE_TEXT)
                    .size(ButtonSize::Small);
                if open.show(ui).clicked() {
                    events.push(ReportEvent::OpenReport(report.report_file.clone()));
                }
            });
    });
}

/// A table row's kind: the header, a stage on the well, or a step indented under it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Row {
    Header,
    Stage,
    Step,
}

/// A step's cells: its title, time, peak RAM (of the app or its worker) and peak VRAM.
fn step_cells(step: StepName, measure: Option<&StepMeasure>) -> [String; 4] {
    let dash = || "—".to_string();
    let mib = |v: Option<f64>| v.map_or_else(dash, |v| format!("{v:.0} MiB"));
    let Some(measure) = measure else {
        return [step_title(step).to_string(), dash(), dash(), dash()];
    };
    let time = if measure.wall_s < 1.0 {
        format!("{:.1} s", measure.wall_s)
    } else {
        format::duration(measure.wall_s)
    };
    let ram = measure
        .peak_ram_mib
        .into_iter()
        .chain(measure.peak_child_ram_mib)
        .reduce(f64::max);
    [
        step_title(step).to_string(),
        time,
        mib(ram),
        mib(measure.peak_vram_mib),
    ]
}

/// One row of the table: the first cell on the left, the numbers right-aligned in their columns,
/// a line above.
fn table_row_ui(ui: &mut Ui, cells: [String; 4], row: Row) {
    let p = palette(ui);
    let (size, colour, bold) = match row {
        Row::Header => (11.0, p.text2, true),
        Row::Stage => (13.0, p.text, true),
        Row::Step => (13.0, p.text, false),
    };
    let width = ui.available_width();
    let galleys: Vec<Arc<Galley>> = cells
        .iter()
        .map(|cell| sized(ui, cell, size, colour, bold))
        .collect();
    let height = 2.0 * TABLE_PAD + galleys.iter().map(|g| g.size().y).fold(0.0, f32::max);
    let (rect, _) = ui.allocate_exact_size(vec2(width, height), Sense::hover());
    if row == Row::Stage {
        ui.painter().rect_filled(rect, 0.0, p.well);
    }
    ui.painter()
        .hline(rect.x_range(), rect.top(), Stroke::new(1.0, p.line));
    let indent = if row == Row::Step { STEP_INDENT } else { SIDE };
    let centre = rect.center().y;
    let mut galleys = galleys.into_iter();
    if let Some(first) = galleys.next() {
        let at = pos2(rect.left() + indent, centre - first.size().y / 2.0);
        ui.painter().galley(at, first, colour);
    }
    let mut right = rect.right() - SIDE - 2.0 * NUMBER_WIDTH;
    for galley in galleys {
        let cell = Rect::from_min_max(
            pos2(right - NUMBER_WIDTH, rect.top()),
            pos2(right, rect.bottom()),
        );
        let at = Align2::RIGHT_CENTER
            .align_size_within_rect(galley.size(), cell)
            .min;
        ui.painter().galley(at, galley, colour);
        right += NUMBER_WIDTH;
    }
}

/// `text` on one line in the proportional font of `size`, semibold when `bold`.
fn sized(ui: &Ui, text: &str, size: f32, colour: Color32, bold: bool) -> Arc<Galley> {
    let family = if bold {
        FontFamily::Name(fonts::SEMIBOLD.into())
    } else {
        FontFamily::Proportional
    };
    ui.painter()
        .layout_no_wrap(text.to_string(), FontId::new(size, family), colour)
}

/// `text` wrapped to `width` in the 13 px proportional font.
fn text(ui: &Ui, text: &str, colour: Color32, width: f32) -> Arc<Galley> {
    ui.painter()
        .layout(text.to_string(), FontId::proportional(13.0), colour, width)
}

//! The selected job's stages: a card with "Show all 29 steps" on top, then one row per stage with
//! its mark, title and time, and under a running or failed stage, or under every stage while the
//! disclosure is open, a line per step.
//!
//! **Role:** draw the stage rows `stage_progress` built: a check and the time a stage took, "so
//! far" on a ring while it runs, "failed" on a red cross, a grey empty ring for a stage to come;
//! each step's line says "already done", "done" with its time when known, a bar while it runs,
//! "running in the background" for the shot scan, "failed" or "to run".
//!
//! **Position:** called by `progress_view` for a running or failed job.
//!
//! **Signals and state:** whether all steps show lives in egui's memory, one per job.
//!
//! **Invariants:** a running or failed stage always shows its steps; a stage row is 40 px high
//! under a 1 px line, and step lines sit 48 px in, 6 px apart.

use std::sync::Arc;

use eframe::egui::text::{LayoutJob, TextWrapping};
use eframe::egui::{
    Align2, Color32, FontId, Galley, Id, Rect, Sense, Stroke, Ui, Vec2, pos2, vec2,
};
use job_model::StepName;

use crate::core::format;
use crate::core::steps::{STAGES, step_title};
use crate::core::ui::card::card;
use crate::core::ui::disclosure::disclosure;
use crate::core::ui::icons::{StatusIcon, paint_status};
use crate::core::ui::palette::palette;
use crate::core::ui::progress::paint_bar;
use crate::job_queue::models::queue::JobId;
use crate::job_queue::services::stage_progress::{StageRow, StageState, StepLine};

/// A stage row's height under its line, its mark, and the space from the card's edges.
const ROW: f32 = 40.0;
const MARK: f32 = 20.0;
const SIDE: f32 = 18.0;
/// The space between a row's parts.
const GAP: f32 = 12.0;
/// A step line's height, the space between lines, its indent and the room under the last one.
const LINE: f32 = 17.0;
const LINE_GAP: f32 = 6.0;
const INDENT: f32 = 48.0;
const BELOW: f32 = 10.0;
/// The widths of a step line's state and time columns.
const STATE_WIDTH: f32 = 120.0;
const TIME_WIDTH: f32 = 70.0;

/// Draw the stages card of job `job` from `rows`.
pub(super) fn stage_list_ui(ui: &mut Ui, job: JobId, rows: &[StageRow]) {
    card(ui, false, |ui| {
        let all = disclosure(
            ui,
            Id::new("all-steps").with(job),
            (
                &format!("Show all {} steps", StepName::ALL.len()),
                &format!("Hide the {} steps", StepName::ALL.len()),
            ),
            &format!("{} stages", STAGES.len()),
        );
        for row in rows {
            stage_row_ui(ui, row);
            let open = matches!(row.state, StageState::Running { .. } | StageState::Failed);
            if all || open {
                steps_ui(ui, &row.steps);
            }
        }
    });
}

/// One stage: the line above it, its mark, its title and its time on the right.
fn stage_row_ui(ui: &mut Ui, row: &StageRow) {
    let p = palette(ui);
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), ROW + 1.0), Sense::hover());
    ui.painter()
        .hline(rect.x_range(), rect.top() + 0.5, Stroke::new(1.0, p.line));
    let centre = rect.top() + 1.0 + ROW / 2.0;
    let (mark, title_colour, time) = match row.state {
        StageState::Kept => (StatusIcon::Done, p.text, "already done".to_string()),
        StageState::Done { seconds } => (
            StatusIcon::Done,
            p.text,
            seconds.map(format::duration).unwrap_or_default(),
        ),
        StageState::Running { share, seconds } => (
            StatusIcon::Running(share),
            p.text,
            format!("{} so far", format::duration(seconds)),
        ),
        StageState::Failed => (StatusIcon::Failed, p.text, "failed".to_string()),
        StageState::Pending | StageState::Background => {
            (StatusIcon::Upcoming, p.text2, String::new())
        }
    };
    let mark_rect = Rect::from_center_size(
        pos2(rect.left() + SIDE + MARK / 2.0, centre),
        Vec2::splat(MARK),
    );
    paint_status(ui, mark_rect, mark, false);
    let time = ui.painter().text(
        pos2(rect.right() - SIDE, centre),
        Align2::RIGHT_CENTER,
        time,
        FontId::proportional(12.0),
        p.text2,
    );
    let left = mark_rect.right() + GAP;
    let title = single_line(
        ui,
        row.stage.title,
        13.0,
        title_colour,
        (time.left() - GAP - left).max(10.0),
    );
    ui.painter().galley(
        pos2(left, centre - title.size().y / 2.0),
        title,
        title_colour,
    );
}

/// A stage's steps: the name, the state and the time of each, on a grid.
fn steps_ui(ui: &mut Ui, steps: &[StepLine]) {
    let p = palette(ui);
    let count = steps.len() as f32;
    let height = count * LINE + (count - 1.0).max(0.0) * LINE_GAP + BELOW;
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let time_right = rect.right() - SIDE;
    let state_left = time_right - TIME_WIDTH - GAP - STATE_WIDTH;
    let name_left = rect.left() + INDENT;
    let name_width = (state_left - GAP - name_left).max(10.0);
    for (i, line) in steps.iter().enumerate() {
        let centre = rect.top() + i as f32 * (LINE + LINE_GAP) + LINE / 2.0;
        let name_colour = if line.state == StageState::Pending {
            p.text2
        } else {
            p.text
        };
        let name = single_line(ui, step_title(line.step), 12.0, name_colour, name_width);
        ui.painter().galley(
            pos2(name_left, centre - name.size().y / 2.0),
            name,
            name_colour,
        );
        let (state, time): (Option<(&str, Color32)>, Option<f64>) = match line.state {
            StageState::Kept => (Some(("already done", p.text2)), None),
            StageState::Pending => (Some(("to run", p.text2)), None),
            StageState::Done { seconds } => (Some(("done", p.good)), seconds),
            StageState::Background => (Some(("running in the background", p.text2)), None),
            StageState::Failed => (Some(("failed", p.bad)), None),
            StageState::Running { share, seconds } => {
                let bar =
                    Rect::from_min_size(pos2(state_left, centre - 2.0), vec2(STATE_WIDTH, 4.0));
                paint_bar(ui, bar, share);
                (None, Some(seconds))
            }
        };
        if let Some((state, colour)) = state {
            ui.painter().text(
                pos2(state_left, centre),
                Align2::LEFT_CENTER,
                state,
                FontId::proportional(12.0),
                colour,
            );
        }
        if let Some(seconds) = time {
            ui.painter().text(
                pos2(time_right, centre),
                Align2::RIGHT_CENTER,
                format::duration(seconds),
                FontId::proportional(12.0),
                p.text2,
            );
        }
    }
}

/// `text` on one line, `size` px, cut with an ellipsis at `width`.
fn single_line(ui: &Ui, text: &str, size: f32, colour: Color32, width: f32) -> Arc<Galley> {
    let mut job =
        LayoutJob::simple_singleline(text.to_string(), FontId::proportional(size), colour);
    job.wrap = TextWrapping::truncate_at_width(width);
    ui.painter().layout_job(job)
}
